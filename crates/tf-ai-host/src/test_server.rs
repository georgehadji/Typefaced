//! Test-only scripted HTTP/1.1 server on 127.0.0.1. It accepts one connection, records
//! the request and answers with a status, headers and a chunked body whose parts can wait
//! for a signal from the test, so streaming and abort can be observed mid-body.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

/// What the server received.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Recorded {
    /// Every value of header `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }
}

/// One part of the response body.
pub enum Part {
    Bytes(&'static [u8]),
    /// Waits until the sender fires (or is dropped) before the next part.
    Wait(oneshot::Receiver<()>),
}

/// The scripted response.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, &'static str)>,
    pub parts: Vec<Part>,
}

impl Reply {
    pub fn ok(body: &'static [u8]) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type", "text/event-stream")],
            parts: vec![Part::Bytes(body)],
        }
    }
}

pub struct MockServer {
    pub origin: String,
    received: Arc<Mutex<Option<Recorded>>>,
}

impl MockServer {
    /// Starts the server; it answers the first connection with `reply`.
    pub async fn start(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&received);
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            *slot.lock().unwrap() = Some(request);
            // Write errors mean the client went away (abort tests): stop quietly.
            let _ = write_reply(&mut stream, reply).await;
        });
        Self { origin, received }
    }

    /// The URL of `path` on this server.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    /// The recorded request, if one arrived.
    pub fn received(&self) -> Option<Recorded> {
        self.received.lock().unwrap().clone()
    }
}

async fn read_request(stream: &mut TcpStream) -> Recorded {
    let mut data = Vec::new();
    let head_end = loop {
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0, "connection closed before the request head ended");
        data.extend_from_slice(&buf[..n]);
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };
    let head = String::from_utf8(data[..head_end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap().split(' ');
    let method = request_line.next().unwrap().to_owned();
    let target = request_line.next().unwrap().to_owned();
    let headers: Vec<(String, String)> = lines
        .map(|line| {
            let (name, value) = line.split_once(':').unwrap();
            (name.trim().to_owned(), value.trim().to_owned())
        })
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0, "connection closed before the request body ended");
        body.extend_from_slice(&buf[..n]);
    }
    Recorded {
        method,
        target,
        headers,
        body: String::from_utf8(body).unwrap(),
    }
}

async fn write_reply(stream: &mut TcpStream, reply: Reply) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} Mock\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("transfer-encoding: chunked\r\nconnection: close\r\n\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.flush().await?;
    for part in reply.parts {
        match part {
            Part::Bytes(bytes) => {
                // An empty chunk would end the body early.
                assert!(!bytes.is_empty(), "body parts must not be empty");
                // One write per chunk, so a part reaches the client in one read.
                let mut chunk = format!("{:x}\r\n", bytes.len()).into_bytes();
                chunk.extend_from_slice(bytes);
                chunk.extend_from_slice(b"\r\n");
                stream.write_all(&chunk).await?;
                stream.flush().await?;
            }
            Part::Wait(signal) => {
                let _ = signal.await;
            }
        }
    }
    stream.write_all(b"0\r\n\r\n").await?;
    stream.flush().await
}
