//! The name suffixes containers answer under: `kiri.local` and
//! `kiri.localhost`, plus the ones `~/.kiri/domains.json` adds, such as
//! `{"suffixes": ["myapp.test"]}`. Kiri's certificate authority is limited
//! to exactly these, so its key can never vouch for any other site.

use std::path::PathBuf;

pub const BUILT_IN: [&str; 2] = ["kiri.local", "kiri.localhost"];

/// Single-label suffixes that no public site can use.
const RESERVED_TLDS: [&str; 4] = ["test", "internal", "lan", "home"];

pub fn path() -> Option<PathBuf> {
    super::state_dir().ok().map(|d| d.join("domains.json"))
}

/// The suffixes `domains.json` adds; none when it is missing or broken.
pub fn extra() -> Vec<String> {
    path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default()
}

/// The built-in suffixes, then the extra ones.
pub fn all() -> Vec<String> {
    let mut v: Vec<String> = BUILT_IN.iter().map(|s| (*s).to_owned()).collect();
    v.extend(extra());
    v
}

/// The valid suffixes of a `domains.json`, lowercased, sorted and without repeats.
pub fn parse(text: &str) -> Vec<String> {
    let json: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let mut v: Vec<String> = json["suffixes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str())
        .map(|s| s.trim().trim_matches('.').to_ascii_lowercase())
        .filter(|s| valid(s) && !BUILT_IN.contains(&s.as_str()))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// A DNS name of letters, digits and hyphens. One label only for a reserved TLD such as `test`.
pub fn valid(s: &str) -> bool {
    let labels: Vec<&str> = s.split('.').collect();
    let label_ok = |l: &&str| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    s.len() <= 200 && labels.iter().all(label_ok) && (labels.len() > 1 || RESERVED_TLDS.contains(&s))
}

/// The name before the longest suffix of `suffixes` that `host` ends with.
pub fn strip_in<'a>(host: &'a str, suffixes: &[String]) -> Option<&'a str> {
    suffixes
        .iter()
        .filter_map(|s| host.len().checked_sub(s.len() + 1).filter(|&at| host.as_bytes()[at] == b'.' && host[at + 1..].eq_ignore_ascii_case(s)).map(|at| &host[..at]))
        .min_by_key(|name| name.len())
}

/// `strip_in` over the current suffixes.
pub fn strip(host: &str) -> Option<&str> {
    strip_in(host, &all())
}

/// The macOS resolver file that sends lookups under `suffix` to Kiri.
pub fn resolver(suffix: &str) -> String {
    format!("/etc/resolver/{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_valid_suffixes() {
        let v = parse(r#"{"suffixes": ["Myapp.Test", "gnosis.test", "gnosis.test", "kiri.local", "com", "bad_name.test", ".lan.", ""]}"#);
        assert_eq!(v, ["gnosis.test", "lan", "myapp.test"]);
        assert!(parse("not json").is_empty());
        assert!(parse(r#"{"suffixes": "test"}"#).is_empty());
    }

    #[test]
    fn single_labels_must_be_reserved() {
        assert!(valid("test") && valid("internal"));
        assert!(!valid("com") && !valid("dev") && !valid("local"));
        assert!(valid("api.example.com"));
        assert!(!valid("-a.test") && !valid("a..test"));
    }

    #[test]
    fn strips_the_longest_matching_suffix() {
        let s = vec!["kiri.local".to_owned(), "kiri.localhost".to_owned(), "test".to_owned(), "gnosis.test".to_owned()];
        assert_eq!(strip_in("api.gnosis.test", &s), Some("api"));
        assert_eq!(strip_in("web.shop.kiri.localhost", &s), Some("web.shop"));
        assert_eq!(strip_in("Web.KIRI.local", &s), Some("Web"));
        assert_eq!(strip_in("kiri.local", &s), None);
        assert_eq!(strip_in("evil.kiri.local.example.com", &s), None);
        assert_eq!(strip_in("notkiri.local", &s), None);
    }
}
