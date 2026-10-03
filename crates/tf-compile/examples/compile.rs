//! Compiles a `.ufo` or `.designspace` to TTF and prints the elapsed milliseconds.
//!
//! `cargo run --release -p tf-compile --example compile -- <input> <output.ttf>`

use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err("usage: compile <input.ufo|input.designspace> <output.ttf>".into());
    };
    let started = Instant::now();
    let font = tf_compile::compile_to_ttf(&PathBuf::from(input))?;
    let elapsed = started.elapsed();
    std::fs::write(output, font)?;
    println!("{}", elapsed.as_millis());
    Ok(())
}
