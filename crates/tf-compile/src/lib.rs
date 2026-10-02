//! Font compiler adapter: compiles UFO and designspace sources to TTF with fontc,
//! linked in-process (ADR-0006). The formal `FontCompiler` port comes in M2.

use std::any::Any;
use std::panic::UnwindSafe;
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

/// Compiles a `.ufo` or `.designspace` source (extension in any case) to TrueType bytes,
/// static or variable, with fontc's default options (the same flags as its command line
/// without arguments; unlike the command line, nothing is written to disk).
///
/// This call blocks and uses several threads: async callers must run it on a blocking
/// thread. The default panic hook still prints a caught panic to stderr, and a stack
/// overflow or an abort cannot be caught.
pub fn compile_to_ttf(source: &Path) -> Result<Vec<u8>, CompileError> {
    let is_supported = source
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("ufo") || ext.eq_ignore_ascii_case("designspace")
        });
    if !is_supported {
        return Err(CompileError::UnsupportedSource(source.to_path_buf()));
    }
    capture_panic(|| compile(source))
}

/// Runs `job` and turns a panic into `CompileError::Panicked`.
///
/// ponytail: only works while the panic strategy is `unwind` (root Cargo.toml). If the
/// crash-handling decision picks `abort`, fontc panics need a process boundary instead.
fn capture_panic<T>(
    job: impl FnOnce() -> Result<T, CompileError> + UnwindSafe,
) -> Result<T, CompileError> {
    std::panic::catch_unwind(job)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn panic_text(job: impl FnOnce() -> Result<(), CompileError> + UnwindSafe) -> String {
        match capture_panic(job) {
            Err(CompileError::Panicked(message)) => message,
            other => format!("expected Panicked, got {other:?}"),
        }
    }

    #[test]
    fn a_str_panic_keeps_its_message() {
        assert_eq!(panic_text(|| panic!("boom")), "boom");
    }

    #[test]
    fn a_formatted_panic_keeps_its_message() {
        let code = 7;
        assert_eq!(panic_text(move || panic!("boom {code}")), "boom 7");
    }

    #[test]
    fn a_panic_with_another_payload_gets_a_generic_message() {
        let message = panic_text(|| std::panic::panic_any(1_u8));
        assert_eq!(message, "unknown panic payload");
    }

    #[test]
    fn a_job_that_does_not_panic_passes_through() {
        assert_eq!(capture_panic(|| Ok(5)).unwrap(), 5);
    }

    #[test]
    fn source_extensions_are_matched_in_any_case() {
        let err = compile_to_ttf(Path::new("font.GLYPHS")).unwrap_err();
        assert!(matches!(err, CompileError::UnsupportedSource(_)));

        let err = compile_to_ttf(Path::new("MISSING.UFO")).unwrap_err();
        assert!(matches!(err, CompileError::Fontc(_)), "{err:?}");
    }
}
