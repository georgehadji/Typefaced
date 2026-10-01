//! Dev-only IPC benchmarks for M0 Step 8 (ADR-0002, ADR-0013).
//!
//! Compiled only with the `bench` Cargo feature, which is never passed to `tauri build`.
//! The commands are raw Tauri commands, outside tauri-specta, so the generated bindings
//! and the product IPC surface stay unchanged. The UI side is `src/bench/` (`#/bench`).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri::ipc::{Channel, Invoke, InvokeBody, InvokeResponseBody, Request, Response};
use tauri::{AppHandle, Manager, State};

/// Every bench command name starts with this prefix; [`crate::run`] routes on it.
pub const COMMAND_PREFIX: &str = "bench_";

/// Results are written here, next to the build output, so they never dirty the tree.
const RESULTS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../target/bench-results.json"
);

/// Upper bound for the results document posted by the page.
const MAX_RESULTS_BYTES: usize = 1 << 20;

/// Upper bounds for one patch-stream run: 2 minutes at 60 Hz, 100,000 points per patch.
const MAX_PATCHES: u32 = 7_200;
const MAX_PATCH_POINTS: u32 = 100_000;
const MIN_INTERVAL_MS: f64 = 1.0;
const MAX_INTERVAL_MS: f64 = 1_000.0;

/// Size of the binary header before the coordinates: `u32` sequence or revision, 4 bytes
/// of padding, then an `f64`, so the coordinates start 8-byte aligned for `Float64Array`.
const HEADER_BYTES: usize = 16;

/// The engine-side outline. A commit swaps in a new `Arc` and leaves the old value
/// untouched, like the persistent-state engine (ADR-0003).
#[derive(Debug, Default)]
struct Outline {
    revision: u32,
    coords: Vec<f64>,
}

/// Benchmark state managed by Tauri.
#[derive(Debug, Default)]
pub struct BenchState {
    glyph: Mutex<Arc<Outline>>,
}

impl BenchState {
    /// The reducer: replaces the glyph outline and returns the new revision.
    fn commit(&self, coords: Vec<f64>) -> Result<Arc<Outline>, String> {
        let mut current = self
            .glyph
            .lock()
            .map_err(|_| "bench state lock poisoned".to_owned())?;
        let next = Arc::new(Outline {
            revision: current.revision.wrapping_add(1),
            coords,
        });
        *current = Arc::clone(&next);
        Ok(next)
    }
}

/// The JSON form of a glyph edit: the whole new outline of the dragged glyph.
#[derive(Debug, Deserialize)]
struct GlyphEdit {
    coords: Vec<f64>,
}

/// Commit round-trip, JSON payloads: JSON edit in, JSON patch `{revision, coords}` out.
#[tauri::command(async)]
fn bench_commit_json(state: State<'_, BenchState>, edit: GlyphEdit) -> Result<Response, String> {
    let next = state.commit(edit.coords)?;
    let patch = serde_json::json!({ "revision": next.revision, "coords": next.coords });
    Ok(Response::new(patch.to_string()))
}

/// Commit round-trip, binary payloads: little-endian `f64` coordinates in, and the patch
/// out as [`HEADER_BYTES`] (revision, padding, unused `f64`) followed by the coordinates.
#[tauri::command(async)]
fn bench_commit_binary(
    state: State<'_, BenchState>,
    request: Request<'_>,
) -> Result<Response, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("bench_commit_binary expects a raw byte body".to_owned());
    };
    let next = state.commit(decode_coords(bytes)?)?;
    Ok(Response::new(encode_packet(
        next.revision,
        0.0,
        &next.coords,
    )))
}

/// Patch stream: sends `count` binary patches of `points` points at `interval_ms` on a
/// background thread. Each patch carries its sequence number and its send time in
/// milliseconds since the Unix epoch, so the page can measure delivery latency.
#[tauri::command]
fn bench_patch_stream(
    on_patch: Channel<InvokeResponseBody>,
    count: u32,
    points: u32,
    interval_ms: f64,
) -> Result<(), String> {
    // The range check also keeps `Duration::from_secs_f64` from panicking.
    let interval_ok = (MIN_INTERVAL_MS..=MAX_INTERVAL_MS).contains(&interval_ms);
    if count > MAX_PATCHES || points > MAX_PATCH_POINTS || !interval_ok {
        return Err("bench_patch_stream arguments out of range".to_owned());
    }
    let coords: Vec<f64> = (0..points * 2).map(f64::from).collect();
    let interval = Duration::from_secs_f64(interval_ms / 1000.0);
    std::thread::spawn(move || {
        let start = Instant::now();
        for seq in 0..count {
            // Absolute deadlines, so sleep overshoot does not accumulate.
            if let Some(wait) = (start + interval * seq).checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            let packet = encode_packet(seq, epoch_ms(), &coords);
            if on_patch.send(InvokeResponseBody::Raw(packet)).is_err() {
                return; // The page went away; nothing left to measure.
            }
        }
    });
    Ok(())
}

/// Receives the results from the page, writes them to `target/bench-results.json` and
/// prints them. Exits the app when `TYPEFACED_BENCH_EXIT=1`, for unattended runs.
#[tauri::command]
fn bench_report(app: AppHandle, results: serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?;
    if text.len() > MAX_RESULTS_BYTES {
        return Err("bench results too large".to_owned());
    }
    std::fs::write(RESULTS_PATH, &text).map_err(|e| format!("cannot write the results: {e}"))?;
    println!("TYPEFACED_BENCH_RESULTS {text}");
    if std::env::var("TYPEFACED_BENCH_EXIT").as_deref() == Ok("1") {
        app.exit(0);
    }
    Ok(())
}

/// The invoke handler for the bench commands.
pub fn invoke_handler() -> impl Fn(Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        bench_commit_json,
        bench_commit_binary,
        bench_patch_stream,
        bench_report
    ]
}

/// Opens the benchmark page (`#/bench`) in the main window.
pub fn open_bench_page(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or("the main window is missing")?;
    let mut url = window.url()?;
    url.set_fragment(Some("/bench"));
    window.navigate(url)?;
    Ok(())
}

fn epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn decode_coords(bytes: &[u8]) -> Result<Vec<f64>, String> {
    let (words, rest) = bytes.as_chunks::<8>();
    if !rest.is_empty() {
        return Err("coordinate bytes must be a multiple of 8".to_owned());
    }
    Ok(words.iter().map(|w| f64::from_le_bytes(*w)).collect())
}

fn encode_packet(word: u32, value: f64, coords: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_BYTES + coords.len() * 8);
    out.extend_from_slice(&word.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&value.to_le_bytes());
    for c in coords {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_round_trip_through_the_binary_packet() {
        let coords = [1.5, -2.0, 1e300];
        let packet = encode_packet(7, 12.25, &coords);
        assert_eq!(packet.len(), HEADER_BYTES + 24);
        assert_eq!(packet[..4], 7u32.to_le_bytes());
        assert_eq!(packet[8..16], 12.25f64.to_le_bytes());
        assert_eq!(decode_coords(&packet[HEADER_BYTES..]).unwrap(), coords);
    }

    #[test]
    fn ragged_coordinate_bytes_are_rejected() {
        assert!(decode_coords(&[0; 9]).is_err());
    }

    #[test]
    fn a_commit_swaps_in_a_new_outline_and_keeps_the_old_one_intact() {
        let state = BenchState::default();
        let first = state.commit(vec![1.0, 2.0]).unwrap();
        let second = state.commit(vec![3.0, 4.0]).unwrap();
        assert_eq!(
            (first.revision, first.coords.as_slice()),
            (1, &[1.0, 2.0][..])
        );
        assert_eq!(
            (second.revision, second.coords.as_slice()),
            (2, &[3.0, 4.0][..])
        );
    }
}
