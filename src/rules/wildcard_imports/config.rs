//! Configuration for `wildcard_imports`: whether each of the two glob
//! exceptions is enabled, the segment names recognised as preludes, and
//! the always-allowed module paths.

use std::collections::BTreeSet;

/// The user-facing configuration shape, deserialised from the
/// `[perfectionist::wildcard_imports]` table of `dylint.toml`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Whether a glob whose final non-glob path segment names a prelude
    /// module (`use rayon::prelude::*;`) is exempt. The recognised
    /// segment names come from `prelude_segment_names`. Defaults to
    /// `true`; set `false` to flag prelude globs too.
    pub(super) prelude_exception: bool,
    /// Whether a bare-`pub` re-export glob (`pub use submodule::*;`) at
    /// the top level of a module body is exempt. Defaults to `true`; set
    /// `false` to flag re-export globs too.
    pub(super) root_reexport_exception: bool,
    /// Path segment names recognised as preludes for the `prelude`
    /// exception. Defaults to `["prelude"]`.
    pub(super) prelude_segment_names: Vec<String>,
    /// Fully-qualified module paths whose glob import is never flagged,
    /// regardless of the exceptions above — the path before the `::*` of
    /// a `use <path>::*`. Defaults to `[]`.
    pub(super) allowed_paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prelude_exception: true,
            root_reexport_exception: true,
            prelude_segment_names: vec!["prelude".to_owned()],
            allowed_paths: Vec::new(),
        }
    }
}

/// The resolved, lookup-ready form of [`Config`] held by the running
/// pass: the two exception toggles carried verbatim and the two name
/// lists interned into sets for membership tests.
pub(super) struct Resolved {
    pub(super) prelude_exception: bool,
    pub(super) root_reexport_exception: bool,
    pub(super) prelude_segment_names: BTreeSet<String>,
    pub(super) allowed_paths: BTreeSet<String>,
}

impl Resolved {
    pub(super) fn from_config(config: Config) -> Self {
        Self {
            prelude_exception: config.prelude_exception,
            root_reexport_exception: config.root_reexport_exception,
            prelude_segment_names: config.prelude_segment_names.into_iter().collect(),
            allowed_paths: config.allowed_paths.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_both_exceptions() {
        let resolved = Resolved::from_config(Config::default());
        assert!(resolved.prelude_exception);
        assert!(resolved.root_reexport_exception);
        assert!(resolved.prelude_segment_names.contains("prelude"));
        assert!(resolved.allowed_paths.is_empty());
    }

    #[test]
    fn both_exceptions_can_be_disabled() {
        let config: Config =
            toml::from_str("prelude_exception = false\nroot_reexport_exception = false").unwrap();
        let resolved = Resolved::from_config(config);
        assert!(!resolved.prelude_exception);
        assert!(!resolved.root_reexport_exception);
    }

    #[test]
    fn one_exception_can_be_disabled_independently() {
        // Disabling one toggle leaves the other at its default. With the
        // old `exceptions = [...]` array this required restating the kept
        // exception; two bools let each be flipped on its own.
        let config: Config = toml::from_str("root_reexport_exception = false").unwrap();
        let resolved = Resolved::from_config(config);
        assert!(resolved.prelude_exception);
        assert!(!resolved.root_reexport_exception);
    }

    #[test]
    fn omitted_fields_fall_back_to_defaults() {
        // A table that sets only `allowed_paths` keeps both exceptions
        // enabled and the default prelude names.
        let config: Config = toml::from_str(r#"allowed_paths = ["foo::bar"]"#).unwrap();
        let resolved = Resolved::from_config(config);
        assert!(resolved.prelude_exception);
        assert!(resolved.root_reexport_exception);
        assert!(resolved.allowed_paths.contains("foo::bar"));
    }

    #[test]
    fn unknown_field_is_rejected() {
        assert!(toml::from_str::<Config>("nonsense = true").is_err());
    }
}
