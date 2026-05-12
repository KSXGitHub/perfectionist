//! Shared helpers for matching macro-invocation paths against configured
//! name lists. Used by every rule that classifies an `ast::MacCall`
//! against a curated name set; see the rules in `src/rules/` that work
//! with `EarlyLintPass::check_mac`.
//!
//! Match semantics:
//!
//! - A single-segment entry matches by the invocation path's final
//!   segment, so `vec!`, `std::vec!`, and `::std::vec!` all match the
//!   `"vec"` entry.
//! - A multi-segment entry tail-matches the invocation path's segment
//!   sequence, so the entry `"inner::their_macro"` matches both
//!   `inner::their_macro!(...)` and `outer::inner::their_macro!(...)`.
//! - Per-segment whitespace is trimmed and empty segments are dropped,
//!   so `"  std::vec  "` is equivalent to `"std::vec"`.

use std::collections::BTreeSet;

/// Split a configured `"a::b::c"`-style path into its segments. Trims
/// per-segment whitespace and drops empty segments — `"  std::vec  "`
/// becomes `["std", "vec"]`. An empty or all-empty input returns an
/// empty vector; callers should treat that as "no matchable path" and
/// skip the entry.
pub(crate) fn parse_path(raw: &str) -> Vec<String> {
    raw.split("::")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parse a list of `"a::b::c"`-style entries from configuration into a
/// deduplicated set of segment sequences. Empty / whitespace-only
/// entries are silently dropped, mirroring the contract of
/// [`parse_path`].
pub(crate) fn parse_path_list(raw_entries: &[String]) -> BTreeSet<Vec<String>> {
    raw_entries
        .iter()
        .map(|entry| parse_path(entry))
        .filter(|parsed| !parsed.is_empty())
        .collect()
}

/// Combine a curated built-in name list (single segments) with a set of
/// user-supplied multi-segment entries into a single matchable set.
/// The built-in side is treated as single-segment entries, which match
/// by the invocation path's final segment.
pub(crate) fn merge_with_builtins(
    builtin: &[&str],
    extras: &BTreeSet<Vec<String>>,
) -> BTreeSet<Vec<String>> {
    builtin
        .iter()
        .map(|name| vec![(*name).to_owned()])
        .chain(extras.iter().cloned())
        .collect()
}

/// Returns `true` if any entry in `entries` matches the invocation path.
pub(crate) fn matches_any(invocation: &rustc_ast::Path, entries: &BTreeSet<Vec<String>>) -> bool {
    entries.iter().any(|entry| entry_matches(entry, invocation))
}

/// Match a configured entry against an invocation path without
/// allocating a `Vec<String>` snapshot of the invocation. Single-segment
/// entries match the path's final segment; multi-segment entries
/// tail-match the path's segment sequence.
pub(crate) fn entry_matches(entry: &[String], invocation: &rustc_ast::Path) -> bool {
    let segments = &invocation.segments;
    if entry.is_empty() || segments.is_empty() {
        return false;
    }
    if entry.len() == 1 {
        return segments
            .last()
            .is_some_and(|segment| segment.ident.name.as_str() == entry[0]);
    }
    if segments.len() < entry.len() {
        return false;
    }
    let start = segments.len() - entry.len();
    segments[start..]
        .iter()
        .zip(entry.iter())
        .all(|(segment, entry_segment)| segment.ident.name.as_str() == entry_segment.as_str())
}
