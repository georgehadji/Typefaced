//! Repository gates. Run with `cargo xtask <task>`; see `CLAUDE.md`.

mod corpus;
mod coverage;
mod deps;
mod licenses;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

const USAGE: &str =
    "usage: cargo xtask <check-deps | licenses-npm | coverage | corpus fetch | corpus verify>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["check-deps"] => check_deps(),
        ["licenses-npm"] => licenses_npm(),
        ["coverage"] => run_coverage(),
        ["corpus", "fetch"] => corpus_fetch(),
        ["corpus", "verify"] => corpus_verify(),
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
    if !root.join("apps/desktop/dist").is_dir() {
        return Err(
            "the desktop crate embeds apps/desktop/dist: run `pnpm --filter @typefaced/desktop build` first"
                .into(),
        );
    }
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

    cargo(&["llvm-cov", "clean", "--workspace"])?;
    cargo(&["llvm-cov", "--workspace", "--locked", "--no-report"])?;
    let mut failed = Vec::new();
    for gate in coverage::gates(&covered) {
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

/// Upper bound for one downloaded archive (1 GiB); the largest pinned one is about 21 MB.
const MAX_ARCHIVE_BYTES: u64 = 1 << 30;

fn corpus_dir() -> PathBuf {
    workspace_root().join("tests").join("corpus")
}

fn corpus_manifest() -> Result<Vec<corpus::Entry>, String> {
    let path = corpus_dir().join("manifest.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    corpus::parse_manifest(&text)
}

fn corpus_fetch() -> Result<(), String> {
    let cache = corpus_dir().join(".cache");
    std::fs::create_dir_all(&cache)
        .map_err(|e| format!("cannot create {}: {e}", cache.display()))?;
    let failures: Vec<String> = corpus_manifest()?
        .iter()
        .filter_map(|entry| fetch_entry(&cache, entry).err())
        .collect();
    report(&failures)?;
    println!("corpus fetch: ok");
    Ok(())
}

fn corpus_verify() -> Result<(), String> {
    let cache = corpus_dir().join(".cache");
    let mut failures = Vec::new();
    for entry in corpus_manifest()? {
        let dir = cache.join(&entry.name);
        let result = if dir.is_dir() {
            tree_digest(&dir).and_then(|actual| corpus::check_digest(&entry, &actual))
        } else {
            Err(format!(
                "corpus {:?} is not fetched: run `cargo xtask corpus fetch`",
                entry.name
            ))
        };
        match result {
            Ok(()) => println!("{}: ok", entry.name),
            Err(e) => failures.push(e),
        }
    }
    report(&failures)?;
    println!("corpus verify: ok");
    Ok(())
}

/// Downloads the pinned archive, extracts only `entry.path` into a staging
/// directory, checks the digest and only then replaces the cached copy.
fn fetch_entry(cache: &Path, entry: &corpus::Entry) -> Result<(), String> {
    let dest = cache.join(&entry.name);
    if dest.symlink_metadata().is_ok() {
        match tree_digest(&dest).and_then(|actual| corpus::check_digest(entry, &actual)) {
            Ok(()) => {
                println!("{}: up to date", entry.name);
                return Ok(());
            }
            Err(e) => println!(
                "{}: fetching again because the cache is stale:\n{e}",
                entry.name
            ),
        }
    }
    // Entry names are `[a-z0-9-]+`, so these never collide with another entry.
    let archive = format!("{}.tar.gz", entry.name);
    let staging = cache.join(format!("{}.partial", entry.name));
    remove_dir(&staging)?;
    println!("{}: downloading {}", entry.name, entry.archive_url());
    let result = download_and_extract(cache, entry, &archive);
    // Cleanup failures are not reported: they would hide `result`, and the next
    // download overwrites the archive and removes the staging directory first.
    let _ = std::fs::remove_file(cache.join(&archive));
    if let Err(e) = result {
        let _ = remove_dir(&staging);
        return Err(e);
    }
    remove_dir(&dest)?;
    std::fs::rename(&staging, &dest).map_err(|e| {
        format!(
            "cannot move {} to {}: {e}",
            staging.display(),
            dest.display()
        )
    })?;
    println!("{}: fetched and verified", entry.name);
    Ok(())
}

/// Paths given to curl and tar are relative to `cache`, their working directory:
/// GNU tar would read `C:\...` as a remote host.
fn download_and_extract(cache: &Path, entry: &corpus::Entry, archive: &str) -> Result<(), String> {
    let url = entry.archive_url();
    let max = MAX_ARCHIVE_BYTES.to_string();
    #[rustfmt::skip]
    let curl = [
        "-q", "-fsSL", "--proto", "=https", "--proto-redir", "=https",
        "--connect-timeout", "30", "--max-time", "1800", "--max-filesize", &max,
        "-o", archive, &url,
    ];
    tool(cache, "curl", &curl)?;
    // `--max-filesize` cannot stop a download without a Content-Length header in
    // older curl versions, so check the size before tar reads the file.
    let size = std::fs::metadata(cache.join(archive))
        .map_err(|e| format!("cannot read {archive}: {e}"))?
        .len();
    if size > MAX_ARCHIVE_BYTES {
        return Err(format!(
            "{archive} is {size} bytes, over the {MAX_ARCHIVE_BYTES}-byte limit"
        ));
    }
    let names = tool(cache, "tar", &["-tzf", archive])?;
    let (member, strip) = corpus::archive_member(entry, names.lines())?;
    corpus::check_entry_types(&tool(cache, "tar", &["-tvzf", archive, "--", &member])?)?;
    let staging = format!("{}.partial", entry.name);
    std::fs::create_dir(cache.join(&staging))
        .map_err(|e| format!("cannot create {staging}: {e}"))?;
    let strip = strip.to_string();
    #[rustfmt::skip]
    let tar = [
        "-xzf", archive, "-C", &staging, "--no-same-owner", "--no-same-permissions",
        "--strip-components", &strip, "--", &member,
    ];
    tool(cache, "tar", &tar)?;
    let actual = tree_digest(&cache.join(&staging))?;
    corpus::check_digest(entry, &actual)
}

/// Runs a tool in `dir` and returns its standard output.
fn tool(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed ({}): {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The corpus digest of the files under `root`. Links (including a linked
/// `root`) and special files are never followed, so nothing outside is read.
fn tree_digest(root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut files = Vec::new();
    let walk = walkdir::WalkDir::new(root)
        .min_depth(1)
        .follow_root_links(false);
    for item in walk {
        let item = item.map_err(|e| format!("cannot walk {}: {e}", root.display()))?;
        let path = item.path();
        if item.file_type().is_dir() {
            continue;
        }
        if !item.file_type().is_file() {
            return Err(format!(
                "{} is not a regular file (links are not allowed in the corpus)",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(root)
            .ok()
            .and_then(|p| p.iter().map(|c| c.to_str()).collect::<Option<Vec<_>>>())
            .ok_or_else(|| format!("{} is not a UTF-8 path under the cache", path.display()))?
            .join("/");
        let mut hasher = Sha256::new();
        let mut file = std::fs::File::open(path)
            .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        std::io::copy(&mut file, &mut hasher)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        files.push((relative, format!("{:x}", hasher.finalize())));
    }
    Ok(corpus::digest(&files))
}

fn remove_dir(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("cannot remove {}: {e}", dir.display()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn scratch(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-{test}-{}", std::process::id()));
        remove_dir(&dir).unwrap();
        std::fs::create_dir_all(dir.join("a").join("empty")).unwrap();
        dir
    }

    #[test]
    fn tree_digest_hashes_files_by_slash_separated_relative_path() {
        let dir = scratch("tree-digest");
        std::fs::write(dir.join("a").join("b.txt"), "x").unwrap();
        std::fs::write(dir.join("c.txt"), "y").unwrap();
        let sha = |s: &str| format!("{:x}", Sha256::digest(s));
        let expected = corpus::digest(&[("a/b.txt".into(), sha("x")), ("c.txt".into(), sha("y"))]);
        assert_eq!(tree_digest(&dir), Ok(expected));
        remove_dir(&dir).unwrap();
        assert_eq!(
            remove_dir(&dir),
            Ok(()),
            "a missing directory is not an error"
        );
    }

    #[cfg(unix)]
    #[test]
    fn tree_digest_rejects_links() {
        let dir = scratch("tree-digest-link");
        std::os::unix::fs::symlink("/etc/hostname", dir.join("a").join("link")).unwrap();
        assert!(
            tree_digest(&dir)
                .unwrap_err()
                .contains("not a regular file")
        );
        remove_dir(&dir).unwrap();
    }

    #[test]
    #[ignore = "needs corpus: cargo xtask corpus fetch"]
    fn cached_corpus_matches_the_manifest() {
        assert_eq!(corpus_verify(), Ok(()));
    }
}
