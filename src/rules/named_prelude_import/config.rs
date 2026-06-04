//! Configuration for `named_prelude_import`: the segment names
//! recognised as preludes and the prelude paths that are never flagged.

use std::collections::BTreeSet;

/// The user-facing configuration shape, deserialised from the
/// `[perfectionist::named_prelude_import]` table of `dylint.toml`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Path segment names recognised as preludes. Matches the knob of
    /// the same name on `perfectionist::wildcard_imports`, so a project
    /// can flip both rules with one value. Defaults to `["prelude"]`.
    pub(super) prelude_segment_names: Vec<String>,
    /// Fully-qualified prelude module paths whose named imports are never
    /// flagged — the module path leading up to and including the prelude
    /// segment (e.g. `crate::prelude`). Useful for a project's own
    /// prelude that is intentionally cherry-picked. Defaults to `[]`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_recognises_prelude() {
        let resolved = Resolved::from_config(Config::default());
        assert!(resolved.prelude_segment_names.contains("prelude"));
        assert!(resolved.allowed_paths.is_empty());
    }

    #[test]
    fn omitted_fields_fall_back_to_defaults() {
        let config: Config = toml::from_str(r#"allowed_paths = ["crate::prelude"]"#).unwrap();
        let resolved = Resolved::from_config(config);
        assert!(resolved.prelude_segment_names.contains("prelude"));
        assert!(resolved.allowed_paths.contains("crate::prelude"));
    }

    #[test]
    fn unknown_field_is_rejected() {
        assert!(toml::from_str::<Config>("nonsense = true").is_err());
    }
}
