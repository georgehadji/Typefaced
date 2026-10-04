//! Compiles a `.ufo` or `.designspace` to TTF, or a `.ufo` to OTF when the output ends
//! in `.otf`, and prints the elapsed milliseconds.
//!
//! `cargo run --release -p tf-compile --example compile -- <input> <output.ttf|output.otf>`

use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err("usage: compile <input.ufo|input.designspace> <output.ttf|output.otf>".into());
    };
    let (input, output) = (PathBuf::from(input), PathBuf::from(output));
    let is_otf = output
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("otf"));
    let started = Instant::now();
    let font = if is_otf {
        tf_compile::compile_to_otf(&input)?
    } else {
        tf_compile::compile_to_ttf(&input)?
    };
    let elapsed = started.elapsed();
    std::fs::write(output, font)?;
    println!("{}", elapsed.as_millis());
    Ok(())
}
