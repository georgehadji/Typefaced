//! `cargo xtask licenses-npm`: production npm packages against the Appendix B
//! allow-list. The list itself is read from `deny.toml`, so Rust and npm share it.

/// Common non-SPDX license strings found in npm metadata, mapped to SPDX.
/// Reviewed by hand; add entries only after checking the package's license text.
const CLARIFY: &[(&str, &str)] = &[
    ("Apache 2.0", "Apache-2.0"),
    ("Apache License 2.0", "Apache-2.0"),
    // Both BSD variants are allowed, so this ambiguity cannot change the verdict.
    ("BSD", "BSD-3-Clause"),
];

/// Reads `[licenses] allow` from the text of `deny.toml`.
pub fn allow_list(deny_toml: &str) -> Result<Vec<String>, String> {
    let config: toml::Table =
        toml::from_str(deny_toml).map_err(|e| format!("cannot read deny.toml: {e}"))?;
    let allow = config
        .get("licenses")
        .and_then(|l| l.get("allow"))
        .and_then(toml::Value::as_array)
        .ok_or("deny.toml has no [licenses] allow list")?;
    allow
        .iter()
        .map(|v| {
            v.as_str()
                .map(String::from)
                .ok_or("deny.toml: allow entries must be strings".into())
        })
        .collect()
}

/// `Ok` when the license expression is satisfied by the allow-list:
/// `OR` needs one allowed alternative, `AND` needs every part.
pub fn check(expression: &str, allowed: &[String]) -> Result<(), String> {
    let text = CLARIFY
        .iter()
        .find(|(from, _)| *from == expression)
        .map_or(expression, |(_, to)| to);
    let parsed = spdx::Expression::parse(text)
        .map_err(|e| format!("cannot parse {expression:?} as SPDX: {e}"))?;
    let licensees = allowed
        .iter()
        .map(|a| {
            spdx::Licensee::parse(a).map_err(|e| format!("allow-list entry {a:?} is not SPDX: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    // The same matching as cargo-deny: `MPL-2.0` satisfies `MPL-2.0+`, and a `WITH`
    // exception is satisfied only by an entry that names it.
    let is_allowed = parsed.evaluate(|req| licensees.iter().any(|l| l.satisfies(req)));
    if is_allowed {
        Ok(())
    } else {
        Err(format!("{expression} is not on the allow-list"))
    }
}

/// Violations in the output of `pnpm licenses list --json`, one line per package.
pub fn npm_violations(pnpm_json: &str, allowed: &[String]) -> Result<Vec<String>, String> {
    let groups: serde_json::Value =
        serde_json::from_str(pnpm_json).map_err(|e| format!("cannot read pnpm output: {e}"))?;
    let groups = groups
        .as_object()
        .ok_or("pnpm output is not an object keyed by license")?;
    let mut found = Vec::new();
    for package in groups
        .values()
        .filter_map(serde_json::Value::as_array)
        .flatten()
    {
        let license = package["license"].as_str().unwrap_or_default();
        if let Err(reason) = check(license, allowed) {
            let name = package["name"].as_str().unwrap_or("<unnamed>");
            let versions: Vec<&str> = package["versions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect();
            found.push(format!("{name}@{}: {reason}", versions.join(", ")));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "MPL-2.0"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn single_allowed_and_denied_licenses() {
        assert_eq!(check("MIT", &allowed()), Ok(()));
        assert_eq!(
            check("GPL-3.0-only", &allowed()),
            Err("GPL-3.0-only is not on the allow-list".into())
        );
    }

    #[test]
    fn or_needs_one_allowed_alternative() {
        assert_eq!(check("GPL-3.0-only OR MIT", &allowed()), Ok(()));
        assert_eq!(check("(Apache-2.0 OR MIT)", &allowed()), Ok(()));
        assert!(check("GPL-3.0-only OR LGPL-2.1-only", &allowed()).is_err());
    }

    #[test]
    fn and_needs_every_part_allowed() {
        assert_eq!(check("MIT AND ISC", &allowed()), Ok(()));
        assert_eq!(
            check("MIT AND GPL-2.0-only", &allowed()),
            Err("MIT AND GPL-2.0-only is not on the allow-list".into())
        );
    }

    #[test]
    fn exceptions_must_be_listed_explicitly_like_cargo_deny() {
        let expression = "Apache-2.0 WITH LLVM-exception";
        assert!(check(expression, &allowed()).is_err());
        let mut with_exception = allowed();
        with_exception.push(expression.into());
        assert_eq!(check(expression, &with_exception), Ok(()));
    }

    #[test]
    fn or_later_requirements_are_met_by_the_listed_version_like_cargo_deny() {
        assert_eq!(check("MPL-2.0+", &allowed()), Ok(()));
        assert!(check("GPL-2.0+", &allowed()).is_err());
    }

    #[test]
    fn clarified_strings_are_mapped_before_parsing() {
        assert_eq!(check("Apache 2.0", &allowed()), Ok(()));
        assert_eq!(check("BSD", &allowed()), Ok(()));
    }

    #[test]
    fn unknown_or_unparseable_licenses_are_violations() {
        assert!(
            check("SEE LICENSE IN LICENSE.md", &allowed())
                .unwrap_err()
                .starts_with("cannot parse \"SEE LICENSE IN LICENSE.md\"")
        );
        assert!(check("Unknown", &allowed()).is_err());
        assert!(check("", &allowed()).is_err());
        assert!(check("LicenseRef-Custom", &allowed()).is_err());
    }

    #[test]
    fn allow_list_is_read_from_deny_toml() {
        let deny = "[licenses]\nallow = [\"MIT\", \"Apache-2.0\"]\n";
        assert_eq!(
            allow_list(deny),
            Ok(vec!["MIT".into(), "Apache-2.0".into()])
        );
        assert!(allow_list("[licenses]\n").is_err());
    }

    #[test]
    fn npm_output_is_checked_per_package() {
        let json = r#"{
            "MIT": [{ "name": "react", "versions": ["19.3.0"], "license": "MIT" }],
            "GPL-3.0": [{ "name": "bad", "versions": ["1.0.0", "1.1.0"], "license": "GPL-3.0" }],
            "Unknown": [{ "name": "odd", "versions": ["0.1.0"], "license": "Unknown" }]
        }"#;
        let mut found = npm_violations(json, &allowed()).unwrap();
        found.sort();
        assert_eq!(found.len(), 2);
        assert!(found[0].starts_with("bad@1.0.0, 1.1.0: "), "{}", found[0]);
        assert!(found[1].starts_with("odd@0.1.0: "), "{}", found[1]);
    }

    #[test]
    fn malformed_npm_output_is_an_error() {
        assert!(npm_violations("not json", &allowed()).is_err());
    }
}
