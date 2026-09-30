//! `cargo xtask coverage`: per-layer line-coverage gates (implementation plan §12.2).

use cargo_metadata::camino::Utf8Path;

use crate::deps::Layer;

/// A workspace crate for coverage grouping.
#[derive(Debug, Clone)]
pub struct CoveredCrate {
    pub name: String,
    pub layer: Layer,
    /// Directory relative to the workspace root, split into path components.
    pub dir: Vec<String>,
}

/// One `cargo llvm-cov report` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    pub label: &'static str,
    pub min_lines_percent: u8,
    pub crates: Vec<String>,
    /// Matches every source file outside the gate's crates; `None` if nothing to exclude.
    pub ignore_filename_regex: Option<String>,
}

/// The directory of `manifest_path`, relative to the workspace root, as path components.
/// An error here would otherwise leave the crate out of every other gate's ignore regex.
pub fn relative_dir(manifest_path: &Utf8Path, root: &Utf8Path) -> Result<Vec<String>, String> {
    let dir = manifest_path
        .parent()
        .and_then(|dir| dir.strip_prefix(root).ok())
        .filter(|dir| !dir.as_str().is_empty())
        .ok_or_else(|| format!("{manifest_path} is outside the workspace root {root}"))?;
    Ok(dir.components().map(|p| p.as_str().to_owned()).collect())
}

/// The gates for the given crates. Groups without crates are skipped;
/// driver and tool crates are not gated.
pub fn gates(crates: &[CoveredCrate]) -> Vec<Gate> {
    const GROUPS: [(&str, u8, &[Layer]); 2] = [
        (
            "domain + application",
            90,
            &[Layer::Domain, Layer::Application],
        ),
        ("adapter", 80, &[Layer::Adapter]),
    ];
    GROUPS
        .into_iter()
        .filter_map(|(label, min_lines_percent, layers)| {
            let (inside, outside): (Vec<_>, Vec<_>) =
                crates.iter().partition(|c| layers.contains(&c.layer));
            if inside.is_empty() {
                return None;
            }
            let ignore: Vec<String> = outside.iter().map(|c| dir_regex(&c.dir)).collect();
            Some(Gate {
                label,
                min_lines_percent,
                crates: inside.iter().map(|c| c.name.clone()).collect(),
                ignore_filename_regex: (!ignore.is_empty()).then(|| ignore.join("|")),
            })
        })
        .collect()
}

/// Matches any file under `dir`, with either path separator.
fn dir_regex(dir: &[String]) -> String {
    const SEP: &str = r"[\\/]";
    let parts: Vec<String> = dir.iter().map(|part| escape(part)).collect();
    format!("{SEP}{}{SEP}", parts.join(SEP))
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if r"\.+*?()|[]{}^$".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, layer: Layer, dir: &str) -> CoveredCrate {
        CoveredCrate {
            name: name.into(),
            layer,
            dir: dir.split('/').map(String::from).collect(),
        }
    }

    #[test]
    fn relative_dir_splits_the_crate_directory_into_components() {
        let root = Utf8Path::new("C:/repo");
        assert_eq!(
            relative_dir(Utf8Path::new("C:/repo/crates/tf-core/Cargo.toml"), root),
            Ok(vec!["crates".into(), "tf-core".into()])
        );
    }

    #[test]
    fn relative_dir_rejects_manifests_outside_the_workspace() {
        let found = relative_dir(
            Utf8Path::new("C:/elsewhere/tf-core/Cargo.toml"),
            Utf8Path::new("C:/repo"),
        );
        assert!(found.unwrap_err().contains("is outside the workspace"));
        assert!(
            relative_dir(
                Utf8Path::new("C:/repo/Cargo.toml"),
                Utf8Path::new("C:/repo")
            )
            .is_err()
        );
    }

    #[test]
    fn domain_and_application_share_a_90_percent_gate() {
        let crates = [
            krate("tf-core", Layer::Domain, "crates/tf-core"),
            krate("tf-commands", Layer::Application, "crates/tf-commands"),
            krate("typefaced-desktop", Layer::Driver, "apps/desktop/src-tauri"),
            krate("xtask", Layer::Tool, "xtask"),
        ];
        assert_eq!(
            gates(&crates),
            [Gate {
                label: "domain + application",
                min_lines_percent: 90,
                crates: vec!["tf-core".into(), "tf-commands".into()],
                ignore_filename_regex: Some(
                    r"[\\/]apps[\\/]desktop[\\/]src-tauri[\\/]|[\\/]xtask[\\/]".into()
                ),
            }]
        );
    }

    #[test]
    fn adapters_get_an_80_percent_gate_that_excludes_other_layers() {
        let crates = [
            krate("tf-core", Layer::Domain, "crates/tf-core"),
            krate("tf-io", Layer::Adapter, "crates/tf-io"),
        ];
        let found = gates(&crates);
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].label, "adapter");
        assert_eq!(found[1].min_lines_percent, 80);
        assert_eq!(found[1].crates, ["tf-io"]);
        assert_eq!(
            found[1].ignore_filename_regex.as_deref(),
            Some(r"[\\/]crates[\\/]tf-core[\\/]")
        );
    }

    #[test]
    fn empty_groups_are_skipped_and_ungated_layers_add_no_gate() {
        let crates = [krate("xtask", Layer::Tool, "xtask")];
        assert_eq!(gates(&crates), []);
    }

    #[test]
    fn a_gate_covering_every_crate_has_nothing_to_ignore() {
        let crates = [krate("tf-core", Layer::Domain, "crates/tf-core")];
        assert_eq!(gates(&crates)[0].ignore_filename_regex, None);
    }

    #[test]
    fn regex_metacharacters_in_paths_are_escaped() {
        let crates = [
            krate("tf-core", Layer::Domain, "crates/tf-core"),
            krate("odd", Layer::Tool, "tools/a.b+c"),
        ];
        assert_eq!(
            gates(&crates)[0].ignore_filename_regex.as_deref(),
            Some(r"[\\/]tools[\\/]a\.b\+c[\\/]")
        );
    }
}
