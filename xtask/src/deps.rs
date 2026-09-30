//! `cargo xtask check-deps`: layer declarations, the crate template and the
//! hexagonal dependency rule (implementation plan §4.2).

use std::collections::HashMap;

/// Architectural layer, declared in `[package.metadata.typefaced] layer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Domain,
    Application,
    Adapter,
    Driver,
    Tool,
}

impl Layer {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "domain" => Some(Self::Domain),
            "application" => Some(Self::Application),
            "adapter" => Some(Self::Adapter),
            "driver" => Some(Self::Driver),
            "tool" => Some(Self::Tool),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Application => "application",
            Self::Adapter => "adapter",
            Self::Driver => "driver",
            Self::Tool => "tool",
        }
    }

    /// Whether a normal or build dependency from `self` to `to` is allowed.
    pub fn may_depend_on(self, to: Layer) -> bool {
        use Layer::*;
        match self {
            Domain => to == Domain,
            Application => matches!(to, Domain | Application),
            Adapter | Driver => matches!(to, Domain | Application | Adapter),
            Tool => true,
        }
    }

    /// External crates this layer must not depend on directly.
    fn banned_externals(self) -> &'static [&'static str] {
        match self {
            Self::Domain => &[
                "tokio",
                "async-std",
                "tauri",
                "reqwest",
                "hyper",
                "keyring",
                "notify",
                "wasm-bindgen",
                "web-sys",
                "js-sys",
            ],
            Self::Application => &["tauri", "reqwest", "hyper", "keyring", "notify"],
            Self::Adapter | Self::Driver | Self::Tool => &[],
        }
    }
}

/// A workspace crate, reduced to what the checks need.
#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    /// The raw `layer` value; `None` when the crate declares none.
    pub layer: Option<String>,
    /// Problems found by [`template_problems`] in the crate's manifest.
    pub template_problems: Vec<String>,
    /// Package names of normal and build dependencies (dev-dependencies are exempt).
    pub dependencies: Vec<String>,
}

/// Checks a parsed `Cargo.toml` against the crate template in the M0 plan.
pub fn template_problems(manifest: &toml::Table) -> Vec<String> {
    let inherits = |table: &str, key: &str| {
        manifest
            .get(table)
            .and_then(|t| t.get(key))
            .and_then(|v| v.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true)
    };
    let mut problems: Vec<String> = ["edition", "rust-version", "license", "publish"]
        .into_iter()
        .filter(|key| !inherits("package", key))
        .map(|key| format!("`{key}` must be `{key}.workspace = true`"))
        .collect();
    let lints_inherited = manifest
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    if !lints_inherited {
        problems.push("missing `[lints] workspace = true`".into());
    }
    problems
}

/// Every rule violation across the workspace, one message each.
pub fn violations(crates: &[Crate]) -> Vec<String> {
    let workspace: HashMap<&str, Option<Layer>> = crates
        .iter()
        .map(|c| (c.name.as_str(), c.layer.as_deref().and_then(Layer::parse)))
        .collect();
    let mut found = Vec::new();
    for c in crates {
        found.extend(
            c.template_problems
                .iter()
                .map(|p| format!("{}: {p}", c.name)),
        );
        let Some(raw) = c.layer.as_deref() else {
            found.push(format!(
                "{}: missing [package.metadata.typefaced] layer",
                c.name
            ));
            continue;
        };
        let Some(from) = Layer::parse(raw) else {
            found.push(format!(
                "{}: unknown layer {raw:?} (expected domain, application, adapter, driver or tool)",
                c.name
            ));
            continue;
        };
        for dep in &c.dependencies {
            match workspace.get(dep.as_str()) {
                // An undeclared or unknown target layer is already reported for that crate.
                Some(Some(to)) if !from.may_depend_on(*to) => found.push(format!(
                    "{} ({}) must not depend on {dep} ({})",
                    c.name,
                    from.name(),
                    to.name()
                )),
                None if from.banned_externals().contains(&dep.as_str()) => found.push(format!(
                    "{} ({}) must not depend on {dep}",
                    c.name,
                    from.name()
                )),
                _ => {}
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, layer: &str, dependencies: &[&str]) -> Crate {
        Crate {
            name: name.into(),
            layer: Some(layer.into()),
            template_problems: Vec::new(),
            dependencies: dependencies.iter().map(|d| (*d).into()).collect(),
        }
    }

    const TEMPLATE: &str = r#"
        [package]
        name = "tf-example"
        version = "0.1.0"
        edition.workspace = true
        rust-version.workspace = true
        license.workspace = true
        publish.workspace = true

        [package.metadata.typefaced]
        layer = "domain"

        [lints]
        workspace = true
    "#;

    fn parse(manifest: &str) -> toml::Table {
        toml::from_str(manifest).unwrap()
    }

    #[test]
    fn layer_rule_matches_the_plan_table() {
        use Layer::*;
        let all = [Domain, Application, Adapter, Driver, Tool];
        let allowed: &[(Layer, &[Layer])] = &[
            (Domain, &[Domain]),
            (Application, &[Domain, Application]),
            (Adapter, &[Domain, Application, Adapter]),
            (Driver, &[Domain, Application, Adapter]),
            (Tool, &all),
        ];
        for (from, targets) in allowed {
            for to in all {
                assert_eq!(
                    from.may_depend_on(to),
                    targets.contains(&to),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn layer_names_parse_and_unknown_names_do_not() {
        assert_eq!(Layer::parse("adapter"), Some(Layer::Adapter));
        assert_eq!(Layer::parse("Domain"), None);
        assert_eq!(Layer::parse("service"), None);
    }

    #[test]
    fn a_clean_workspace_has_no_violations() {
        let crates = [
            krate("tf-core", "domain", &["serde"]),
            krate("tf-commands", "application", &["tf-core", "serde"]),
            krate("typefaced-desktop", "driver", &["tf-commands", "tauri"]),
            krate("xtask", "tool", &["typefaced-desktop", "tf-core"]),
        ];
        assert_eq!(violations(&crates), Vec::<String>::new());
    }

    #[test]
    fn inward_edges_to_outer_layers_are_reported() {
        let crates = [
            krate("tf-core", "domain", &["tf-commands"]),
            krate("tf-commands", "application", &[]),
            krate("tf-io", "adapter", &["typefaced-desktop"]),
            krate("typefaced-desktop", "driver", &[]),
        ];
        assert_eq!(
            violations(&crates),
            [
                "tf-core (domain) must not depend on tf-commands (application)",
                "tf-io (adapter) must not depend on typefaced-desktop (driver)",
            ]
        );
    }

    #[test]
    fn banned_external_crates_are_reported_per_layer() {
        let crates = [
            krate("tf-core", "domain", &["tokio", "wasm-bindgen"]),
            krate("tf-commands", "application", &["tauri", "tokio"]),
            krate("tf-io", "adapter", &["tokio", "notify"]),
        ];
        assert_eq!(
            violations(&crates),
            [
                "tf-core (domain) must not depend on tokio",
                "tf-core (domain) must not depend on wasm-bindgen",
                "tf-commands (application) must not depend on tauri",
            ]
        );
    }

    #[test]
    fn missing_or_unknown_layers_are_reported() {
        let mut undeclared = krate("tf-a", "domain", &[]);
        undeclared.layer = None;
        let crates = [undeclared, krate("tf-b", "service", &[])];
        assert_eq!(
            violations(&crates),
            [
                "tf-a: missing [package.metadata.typefaced] layer",
                "tf-b: unknown layer \"service\" (expected domain, application, adapter, driver or tool)",
            ]
        );
    }

    #[test]
    fn edges_from_or_to_undeclared_crates_are_not_double_reported() {
        let mut undeclared = krate("tf-a", "domain", &[]);
        undeclared.layer = None;
        let crates = [undeclared, krate("tf-b", "domain", &["tf-a"])];
        assert_eq!(
            violations(&crates),
            ["tf-a: missing [package.metadata.typefaced] layer"]
        );
    }

    #[test]
    fn template_problems_are_reported_with_the_crate_name() {
        let mut crate_ = krate("tf-a", "domain", &[]);
        crate_.template_problems = vec!["`edition` must be `edition.workspace = true`".into()];
        assert_eq!(
            violations(&[crate_]),
            ["tf-a: `edition` must be `edition.workspace = true`"]
        );
    }

    #[test]
    fn the_template_itself_passes() {
        assert_eq!(template_problems(&parse(TEMPLATE)), Vec::<String>::new());
    }

    #[test]
    fn literal_package_keys_and_missing_lints_are_reported() {
        let manifest = parse(
            r#"
            [package]
            name = "tf-example"
            version = "0.1.0"
            edition = "2024"
            rust-version.workspace = true
            license = "MIT"
            "#,
        );
        assert_eq!(
            template_problems(&manifest),
            [
                "`edition` must be `edition.workspace = true`",
                "`license` must be `license.workspace = true`",
                "`publish` must be `publish.workspace = true`",
                "missing `[lints] workspace = true`",
            ]
        );
    }
}
