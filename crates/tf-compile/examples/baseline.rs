//! Size baseline for Spike 1: the same shape as `compile`, without fontc.
//! Copies `<input>` to `<output>` and prints the elapsed milliseconds.

use std::error::Error;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err("usage: baseline <input file> <output file>".into());
    };
    let started = Instant::now();
    std::fs::copy(input, output)?;
    println!("{}", started.elapsed().as_millis());
    Ok(())
}
