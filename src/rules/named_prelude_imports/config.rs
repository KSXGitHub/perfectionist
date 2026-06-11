//! Configuration for `named_prelude_imports`: the segment names
//! recognised as preludes and the prelude paths that are never flagged.

use std::collections::BTreeSet;

/// The user-facing configuration shape, deserialised from the
/// `[perfectionist::named_prelude_imports]` table of `dylint.toml`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Path segment names recognised as preludes. Matches the knob of
    /// the same name on `perfectionist::wildcard_imports`, so a project
    /// can flip both rules with one value. Defaults to `["prelude"]`.
    pub(super) prelude_segment_names: Vec<String>,
    /// Prelude module paths whose named imports are never flagged — the
    /// module path leading up to and including the prelude segment. Each
    /// entry is **absolute** and written with a leading `::` (e.g.
    /// `"::crate::prelude"`); the leading `::` matches whether or not the
    /// `use` writes it. An entry without the leading `::` is reserved for
    /// future relative (suffix) matching and currently matches nothing.
    /// Because an entry matches the path up to *and including* the prelude
    /// segment, its final segment must be one of `prelude_segment_names` —
    /// an entry that ends in anything else can never match a cherry-pick
    /// and is rejected as a configuration error. Matching is exact and
    /// syntactic: the entry must equal the path as written in the `use`
    /// (up to the prelude segment), with no re-export or alias resolution.
    /// Useful for a project's own prelude that is intentionally
    /// cherry-picked. Defaults to `[]`.
    pub(super) allowed_paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prelude_segment_names: vec!["prelude".to_owned()],
            allowed_paths: Vec::new(),
        }
    }
}

/// The resolved, lookup-ready form of [`Config`] held by the running
/// pass: both name lists interned into sets for membership tests.
pub(super) struct Resolved {
    pub(super) prelude_segment_names: BTreeSet<String>,
    pub(super) allowed_paths: BTreeSet<String>,
}

impl Resolved {
    pub(super) fn from_config(config: Config) -> Self {
        Self {
            prelude_segment_names: config.prelude_segment_names.into_iter().collect(),
            allowed_paths: config.allowed_paths.into_iter().collect(),
        }
    }
}

/// Validate that every `allowed_paths` entry ends with a recognised
/// prelude segment. An entry matches the module path up to and including
/// the prelude segment, so its final segment must be one of
/// `prelude_segment_names`; an entry that ends in anything else can never
/// match a cherry-pick and is almost certainly a mistake. Returns an
/// error message — for the dylint run to fail on — naming the first
/// offending entry.
pub(super) fn validate(config: &Config) -> Result<(), String> {
    let names: BTreeSet<&str> = config
        .prelude_segment_names
        .iter()
        .map(String::as_str)
        .collect();
    for entry in &config.allowed_paths {
        let last = entry
            .split("::")
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .last();
        if !last.is_some_and(|segment| names.contains(segment)) {
            return Err(format!(
                "`allowed_paths` entry {entry:?} must end with a prelude segment name \
                 (one of {names:?}); an entry matches the module path up to and \
                 including the prelude segment, so its final segment has to be in \
                 `prelude_segment_names`",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
