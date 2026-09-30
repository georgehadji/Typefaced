//! Repository gates. Run with `cargo xtask <task>`; see `CLAUDE.md`.

mod coverage;
mod deps;
mod licenses;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

const USAGE: &str = "usage: cargo xtask <check-deps | licenses-npm | coverage>";

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("check-deps") => check_deps(),
        Some("licenses-npm") => licenses_npm(),
        Some("coverage") => run_coverage(),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn metadata() -> Result<Metadata, String> {
    MetadataCommand::new()
        .manifest_path(workspace_root().join("Cargo.toml"))
        .no_deps()
        .exec()
        .map_err(|e| format!("cargo metadata failed: {e}"))
}

fn workspace_crates(metadata: &Metadata) -> Result<Vec<deps::Crate>, String> {
    metadata
        .workspace_packages()
        .into_iter()
        .map(|package| {
            let manifest = std::fs::read_to_string(&package.manifest_path)
                .map_err(|e| format!("cannot read {}: {e}", package.manifest_path))?;
            let manifest: toml::Table = toml::from_str(&manifest)
                .map_err(|e| format!("cannot parse {}: {e}", package.manifest_path))?;
            Ok(deps::Crate {
                name: package.name.clone(),
                layer: package.metadata["typefaced"]["layer"]
                    .as_str()
                    .map(String::from),
                template_problems: deps::template_problems(&manifest),
                dependencies: package
                    .dependencies
                    .iter()
                    .filter(|d| d.kind != DependencyKind::Development)
                    .map(|d| d.name.clone())
                    .collect(),
            })
        })
        .collect()
}

fn report(violations: &[String]) -> Result<(), String> {
    for violation in violations {
        eprintln!("  {violation}");
    }
    match violations.len() {
        0 => Ok(()),
        n => Err(format!("{n} violation(s)")),
    }
}

fn check_deps() -> Result<(), String> {
    let crates = workspace_crates(&metadata()?)?;
    report(&deps::violations(&crates))?;
    println!("check-deps: {} workspace crates ok", crates.len());
    Ok(())
}

fn licenses_npm() -> Result<(), String> {
    let root = workspace_root();
    let deny = std::fs::read_to_string(root.join("deny.toml"))
        .map_err(|e| format!("cannot read deny.toml: {e}"))?;
    let allowed = licenses::allow_list(&deny)?;
    // pnpm is a `.cmd` shim on Windows, which `Command` does not resolve by itself.
    let pnpm = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
    let output = Command::new(pnpm)
        .args(["licenses", "list", "--json", "--prod"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("cannot run pnpm: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "pnpm licenses list failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json = String::from_utf8_lossy(&output.stdout);
    report(&licenses::npm_violations(&json, &allowed)?)?;
    println!("licenses-npm: production npm packages ok");
    Ok(())
}

fn run_coverage() -> Result<(), String> {
    let metadata = metadata()?;
    let crates = workspace_crates(&metadata)?;
    report(&deps::violations(&crates))?;
    let root = &metadata.workspace_root;
    let covered = metadata
        .workspace_packages()
        .into_iter()
        .zip(&crates)
        .map(|(package, c)| {
            Ok(coverage::CoveredCrate {
                name: c.name.clone(),
                // check-deps above guarantees every layer parses.
                layer: c
                    .layer
                    .as_deref()
                    .and_then(deps::Layer::parse)
                    .unwrap_or(deps::Layer::Tool),
                dir: coverage::relative_dir(&package.manifest_path, root)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let gates = coverage::gates(&covered);
    // Only gated crates are built and tested, so the driver (Tauri, WebView) never is:
    // this runs on Linux CI without WebKitGTK and without the frontend build.
    let mut run = vec!["llvm-cov", "--locked", "--no-report"];
    for name in gates.iter().flat_map(|gate| &gate.crates) {
        run.extend(["--package", name]);
    }
    if gates.is_empty() {
        println!("coverage: no gated crates yet");
        return Ok(());
    }
    cargo(&["llvm-cov", "clean", "--workspace"])?;
    cargo(&run)?;
    let mut failed = Vec::new();
    for gate in gates {
        println!(
            "\ncoverage gate {} (>= {}% lines): {}",
            gate.label,
            gate.min_lines_percent,
            gate.crates.join(", ")
        );
        let min = gate.min_lines_percent.to_string();
        let mut args = vec!["llvm-cov", "report", "--fail-under-lines", &min];
        if let Some(regex) = &gate.ignore_filename_regex {
            args.extend(["--ignore-filename-regex", regex]);
        }
        if cargo(&args).is_err() {
            failed.push(gate.label);
        }
    }
    match failed.as_slice() {
        [] => Ok(()),
        labels => Err(format!("coverage below target: {}", labels.join(", "))),
    }
}

fn cargo(args: &[&str]) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .args(args)
        .current_dir(workspace_root())
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed ({status})", args.join(" ")))
    }
}
