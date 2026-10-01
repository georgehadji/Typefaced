//! Font compiler adapter: compiles UFO and designspace sources to TTF with fontc,
//! linked in-process (ADR-0006). The formal `FontCompiler` port comes in M2.

use std::any::Any;
use std::path::{Path, PathBuf};

use fontc::{Input, Options};

/// Why a compile failed.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// The path is not a `.ufo` or `.designspace` source.
    #[error("unsupported source '{}': expected a .ufo or .designspace", .0.display())]
    UnsupportedSource(PathBuf),
    /// fontc could not read or compile the source.
    #[error(transparent)]
    Fontc(#[from] fontc::Error),
    /// fontc panicked. fontc 1.0.0 does this on some inputs it does not support,
    /// such as a designspace 5 discrete axis.
    #[error("fontc panicked: {0}")]
    Panicked(String),
}

/// Compiles a `.ufo` or `.designspace` source to TrueType bytes (static or variable),
/// with fontc's default options (the same as its command line without flags).
pub fn compile_to_ttf(source: &Path) -> Result<Vec<u8>, CompileError> {
    let extension = source.extension().and_then(|ext| ext.to_str());
    if !matches!(extension, Some("ufo" | "designspace")) {
        return Err(CompileError::UnsupportedSource(source.to_path_buf()));
    }
    // ponytail: only works while the panic strategy is `unwind` (root Cargo.toml). If the
    // crash-handling decision picks `abort`, fontc panics need a process boundary instead.
    std::panic::catch_unwind(|| compile(source))
        .unwrap_or_else(|payload| Err(CompileError::Panicked(panic_message(payload.as_ref()))))
}

fn compile(source: &Path) -> Result<Vec<u8>, CompileError> {
    let source = Input::new(source)?.create_source()?;
    Ok(fontc::generate_font(source, Options::default())?)
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic payload".to_owned())
}
