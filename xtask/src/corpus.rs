//! `cargo xtask corpus fetch | verify`: the pinned font corpus in
//! `tests/corpus/manifest.toml`. This module holds the pure parts (manifest
//! checks, archive listing checks, the digest); `main.rs` downloads and extracts.

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Licenses a corpus source may carry (they allow test use).
pub const LICENSES: [&str; 3] = ["OFL-1.1", "Apache-2.0", "MIT"];

/// One pinned source: a directory inside a GitHub repository at a fixed commit.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub repo: String,
    pub commit: String,
    pub path: String,
    pub license: String,
    pub digest: String,
    pub purpose: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    entry: Vec<Entry>,
}

/// Parses and validates the text of `manifest.toml`.
pub fn parse_manifest(text: &str) -> Result<Vec<Entry>, String> {
    let manifest: Manifest =
        toml::from_str(text).map_err(|e| format!("cannot read the corpus manifest: {e}"))?;
    let licenses = LICENSES.map(String::from);
    let mut seen = std::collections::HashSet::new();
    for entry in &manifest.entry {
        validate(entry, &licenses).map_err(|e| format!("manifest entry {:?}: {e}", entry.name))?;
        if !seen.insert(&entry.name) {
            return Err(format!("manifest entry {:?} is a duplicate", entry.name));
        }
    }
    Ok(manifest.entry)
}

fn validate(entry: &Entry, licenses: &[String]) -> Result<(), String> {
    if !is_name(&entry.name) {
        return Err(
            "`name` must be lowercase ASCII letters, digits and '-', and not a Windows device name"
                .into(),
        );
    }
    if !matches!(entry.repo.split('/').collect::<Vec<_>>()[..], [owner, name] if is_segment(owner) && is_segment(name))
    {
        return Err(format!("`repo` {:?} must be `owner/name`", entry.repo));
    }
    if !is_lower_hex(&entry.commit, 40) {
        return Err(format!(
            "`commit` {:?} must be a full 40-character SHA",
            entry.commit
        ));
    }
    if !entry.path.split('/').all(is_segment) {
        return Err(format!(
            "`path` {:?} must be a relative directory path of ASCII letters, digits, '.', '_' and '-'",
            entry.path
        ));
    }
    crate::licenses::check(&entry.license, licenses).map_err(|e| {
        format!(
            "`license`: {e} (corpus sources must be {})",
            LICENSES.join(", ")
        )
    })?;
    if !is_lower_hex(&entry.digest, 64) {
        return Err("`digest` must be 64 lowercase hex digits (SHA-256)".into());
    }
    Ok(())
}

/// A cache directory name that is also safe as `<name>.tar.gz` on Windows.
fn is_name(s: &str) -> bool {
    let device = matches!(s, "con" | "prn" | "aux" | "nul")
        || (s.len() == 4
            && (s.starts_with("com") || s.starts_with("lpt"))
            && s.as_bytes()[3].is_ascii_digit());
    !s.is_empty()
        && !s.starts_with('-')
        && !device
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A path segment with no special meaning to shells, URLs, tar patterns or
/// Windows (which drops a trailing '.').
fn is_segment(s: &str) -> bool {
    !s.is_empty()
        && !s.ends_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Entry {
    /// The GitHub archive of the pinned commit.
    pub fn archive_url(&self) -> String {
        format!(
            "https://codeload.github.com/{}/tar.gz/{}",
            self.repo, self.commit
        )
    }
}

/// Checks the names listed by `tar -tzf` and returns the member to extract
/// (`<repo name>-<commit>/<path>`) and the number of leading components to strip.
/// Defence in depth: tar refuses `..` and absolute names by default, but it is
/// never asked to extract one.
pub fn archive_member<'a>(
    entry: &Entry,
    names: impl IntoIterator<Item = &'a str>,
) -> Result<(String, usize), String> {
    let repo_name = entry.repo.rsplit('/').next().unwrap_or_default();
    let top_dir = format!("{repo_name}-{}", entry.commit);
    let member = format!("{top_dir}/{}", entry.path);
    let mut found = false;
    let mut empty = true;
    for name in names {
        empty = false;
        let outside =
            name.split('/').next() != Some(top_dir.as_str()) || name.split('/').any(|c| c == "..");
        let below = name
            .strip_prefix(&member)
            .filter(|rest| rest.starts_with('/'));
        let unsafe_below =
            below.is_some() && name.contains(|c: char| c == '\\' || c == ':' || c.is_control());
        if outside || unsafe_below {
            return Err(format!(
                "the archive has an unexpected entry name: {name:?}"
            ));
        }
        found |= below.is_some();
    }
    if empty {
        return Err("the archive is empty".into());
    }
    if !found {
        return Err(format!(
            "`{}` is not a directory in the archive",
            entry.path
        ));
    }
    Ok((member, 1 + entry.path.split('/').count()))
}

/// Checks `tar -tvzf` output: only regular files (`-`) and directories (`d`),
/// so no link can make tar write outside the staging directory. GNU tar and
/// bsdtar both start each line with the `ls -l` type character.
pub fn check_entry_types(verbose_listing: &str) -> Result<(), String> {
    match verbose_listing
        .lines()
        .find(|line| !line.is_empty() && !line.starts_with(['-', 'd']))
    {
        Some(line) => Err(format!(
            "the archive has an entry that is not a file or directory: {line}"
        )),
        None => Ok(()),
    }
}
/// SHA-256 over the sorted lines `<relative path>\t<sha256 of file>\n`.
pub fn digest(files: &[(String, String)]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .map(|(path, sha)| format!("{path}\t{sha}\n"))
        .collect();
    lines.sort();
    format!("{:x}", Sha256::digest(lines.concat()))
}

/// `Ok` when the digest of the extracted files matches the manifest.
pub fn check_digest(entry: &Entry, actual: &str) -> Result<(), String> {
    if actual == entry.digest {
        return Ok(());
    }
    Err(format!(
        "corpus {:?}: digest mismatch\n  manifest: {}\n  files:    {actual}\n  \
         The files differ from the pinned source. Run `cargo xtask corpus fetch` to \
         restore them, or update the manifest if the pin changed on purpose.",
        entry.name, entry.digest
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// TOML literal strings, so backslashes reach the validator unchanged.
    fn manifest(name: &str, repo: &str, commit: &str, path: &str, license: &str) -> String {
        format!(
            "[[entry]]\nname = '{name}'\nrepo = '{repo}'\ncommit = '{commit}'\n\
             path = '{path}'\nlicense = '{license}'\ndigest = '{DIGEST}'\npurpose = 'test'\n"
        )
    }

    fn valid() -> String {
        manifest(
            "inria-sans",
            "Owner/Repo.name_1",
            SHA,
            "masters/INRIA-SANS",
            "OFL-1.1",
        )
    }

    fn entry() -> Entry {
        parse_manifest(&valid()).unwrap().remove(0)
    }

    fn entry_with_path(path: &str) -> Entry {
        Entry {
            path: path.into(),
            ..entry()
        }
    }

    fn top() -> String {
        format!("Repo.name_1-{SHA}")
    }

    fn rejects(text: &str, field: &str) {
        let error = parse_manifest(text).unwrap_err();
        assert!(error.contains(&format!("`{field}`")), "{text}\n{error}");
    }

    #[test]
    fn a_valid_manifest_parses() {
        assert_eq!(
            parse_manifest(&valid()),
            Ok(vec![Entry {
                name: "inria-sans".into(),
                repo: "Owner/Repo.name_1".into(),
                commit: SHA.into(),
                path: "masters/INRIA-SANS".into(),
                license: "OFL-1.1".into(),
                digest: DIGEST.into(),
                purpose: "test".into(),
            }])
        );
    }

    #[test]
    fn the_committed_manifest_is_valid() {
        let entries = parse_manifest(include_str!("../../tests/corpus/manifest.toml")).unwrap();
        assert!(!entries.is_empty());
    }

    #[test]
    fn unknown_or_missing_fields_are_rejected() {
        assert!(parse_manifest(&format!("{}extra = 1\n", valid())).is_err());
        assert!(parse_manifest(&valid().replace("purpose = 'test'\n", "")).is_err());
        assert!(parse_manifest("").is_err());
    }

    #[test]
    fn names_must_be_safe_directory_names() {
        for bad in [
            "", "..", ".", "a/b", "a\\b", "C:", "-a", "A", "a.b", "a b", "nul", "con", "com1",
            "lpt9",
        ] {
            rejects(&manifest(bad, "o/r", SHA, "p", "MIT"), "name");
        }
        assert!(parse_manifest(&manifest("console-1", "o/r", SHA, "p", "MIT")).is_ok());
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let text = format!("{}{}", valid(), valid());
        assert!(parse_manifest(&text).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn repo_must_be_owner_slash_name() {
        for bad in [
            "", "owner", "o/r/x", "/r", "o/", "o/..", "../r", "o/r?x", "o/r#x", "o r/x", "o\\r",
        ] {
            rejects(&manifest("a", bad, SHA, "p", "MIT"), "repo");
        }
    }

    #[test]
    fn commit_must_be_a_full_lowercase_sha() {
        let short = &SHA[..39];
        let upper = SHA.to_uppercase();
        for bad in ["", "main", short, &upper, &format!("{SHA}0")] {
            rejects(&manifest("a", "o/r", bad, "p", "MIT"), "commit");
        }
    }

    #[test]
    fn path_must_be_a_plain_relative_path() {
        for bad in [
            "", "/abs", "a/../b", "..", "./a", "a//b", "a/", "a\\b", "C:/x", "a*", "a?b", "[a]",
            "a b", "a./b", "a/b.",
        ] {
            rejects(&manifest("a", "o/r", SHA, bad, "MIT"), "path");
        }
    }

    #[test]
    fn license_must_be_one_of_the_corpus_licenses() {
        for good in ["OFL-1.1", "Apache-2.0", "MIT", "Apache-2.0 AND OFL-1.1"] {
            assert!(
                parse_manifest(&manifest("a", "o/r", SHA, "p", good)).is_ok(),
                "{good}"
            );
        }
        for bad in [
            "GPL-3.0-only",
            "BSD-3-Clause",
            "OFL-1.1 AND GPL-2.0-only",
            "",
            "Proprietary",
        ] {
            rejects(&manifest("a", "o/r", SHA, "p", bad), "license");
        }
    }

    #[test]
    fn digest_must_be_lowercase_sha256_hex() {
        for bad in ["", "abc", &DIGEST.to_uppercase(), &DIGEST[1..]] {
            rejects(&valid().replace(DIGEST, bad), "digest");
        }
    }

    #[test]
    fn archive_url_points_at_the_pinned_commit() {
        assert_eq!(
            entry().archive_url(),
            format!("https://codeload.github.com/Owner/Repo.name_1/tar.gz/{SHA}")
        );
    }

    #[test]
    fn archive_member_is_the_path_under_the_repo_commit_directory() {
        let t = top();
        let names = [
            format!("{t}/"),
            format!("{t}/README.md"),
            format!("{t}/other/a:b"),
            format!("{t}/masters/INRIA-SANS/"),
            format!("{t}/masters/INRIA-SANS/A.ufo/glyphs.{{600, 400}}/A_.glif"),
        ];
        assert_eq!(
            archive_member(&entry(), names.iter().map(String::as_str)),
            Ok((format!("{t}/masters/INRIA-SANS"), 3))
        );
    }

    #[test]
    fn archive_member_must_exist() {
        let t = top();
        let names = [format!("{t}/"), format!("{t}/masters/INRIA-SANSX/a")];
        let error = archive_member(&entry(), names.iter().map(String::as_str)).unwrap_err();
        assert!(error.contains("masters/INRIA-SANS"), "{error}");
        assert!(archive_member(&entry(), []).is_err());
    }

    #[test]
    fn archives_with_unsafe_or_foreign_names_are_rejected() {
        let t = top();
        for bad in [
            format!("{t}/p/../../evil"),
            format!("{t}/other/../../evil"),
            format!("{t}/../evil"),
            "/etc/passwd".into(),
            format!("{t}/p/a\\..\\b"),
            format!("{t}/p/c:evil"),
            format!("{t}/p/tab\there"),
            format!("{t}/p/line\nbreak"),
            "C:/evil".into(),
            "other/p/x".into(),
            "--to-command=x/p/y".into(),
            String::new(),
        ] {
            let names = [format!("{t}/p/ok"), bad.clone()];
            let result = archive_member(&entry_with_path("p"), names.iter().map(String::as_str));
            assert!(result.is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_regular_files_and_directories_may_be_extracted() {
        let gnu = "drwxrwxr-x root/root 0 2021-12-08 07:49 t/p/\n\
                   -rw-rw-r-- root/root 5 2021-12-08 07:49 t/p/a b.txt\n";
        let bsd = "drwxrwxr-x  0 root   root        0 Dec 08  2021 t/p/\r\n\
                   -rw-rw-r--  0 root   root    63966 Dec 08  2021 t/p/x\r\n";
        assert_eq!(check_entry_types(gnu), Ok(()));
        assert_eq!(check_entry_types(bsd), Ok(()));
        for kind in ['l', 'h', 'c', 'b', 'p', 's'] {
            let listing =
                format!("{gnu}{kind}rwxrwxrwx root/root 0 2021-12-08 07:49 t/p/link -> /etc\n");
            let error = check_entry_types(&listing).unwrap_err();
            assert!(error.contains("t/p/link"), "{error}");
        }
    }

    #[test]
    fn digest_is_sha256_of_sorted_lines() {
        // Independent value: `printf 'a/b\t11\nb\t22\n' | sha256sum`.
        let files = [
            ("b".to_string(), "22".to_string()),
            ("a/b".into(), "11".into()),
        ];
        assert_eq!(
            digest(&files),
            "f7995e70082a458c0d42b09993ede91ddc29dafc9521a8317519cf097e2bc89b"
        );
        assert_eq!(digest(&[]), DIGEST);
    }

    #[test]
    fn digest_mismatch_names_the_entry_and_both_values() {
        let e = entry();
        assert_eq!(check_digest(&e, DIGEST), Ok(()));
        let error = check_digest(&e, "ff").unwrap_err();
        assert!(error.contains("inria-sans"), "{error}");
        assert!(error.contains(DIGEST) && error.contains("ff"), "{error}");
    }
}
