//! Configuration and in-memory state for
//! `prefer_derive_more_over_thiserror`: the user-facing `Config`
//! shape, the default path list, the `PreferDeriveMoreOverThiserror`
//! pass state, and the configured-path matcher shared by the
//! [`scan`](super::scan) and [`detect`](super::detect) submodules.

use std::collections::{BTreeMap, BTreeSet};

use rustc_span::Symbol;

const CONFIG_KEY: &str = "perfectionist::prefer_derive_more_over_thiserror";

/// Recognised `thiserror` derive paths. The default covers the
/// canonical crate; a project that re-publishes the derive under a
/// different crate name can extend the list.
const DEFAULT_THISERROR_PATHS: &[&str] = &["thiserror::Error"];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Paths whose presence in a `#[derive(...)]` list (or whose
    /// crate's presence in a `use` statement) flags the site. Each
    /// entry is a `::`-separated path string. Replaces the default
    /// `["thiserror::Error"]` when supplied; the empty list `[]`
    /// disables the rule.
    thiserror_paths: Option<Vec<String>>,
}

/// Parsed configuration plus the crate-wide alias maps the pass
/// accumulates in its pre-expansion `check_crate` scan. Fields are
/// `pub(super)` so the sibling `scan` and `detect` submodules (both
/// descendants of the rule's entry module) can read and, in the
/// scan's case, populate them.
pub(super) struct PreferDeriveMoreOverThiserror {
    /// Configured paths split into segment lists (e.g.
    /// `[[thiserror, Error]]`).
    pub(super) thiserror_paths: Vec<Vec<Symbol>>,
    /// First segments of every configured path — the crate names a
    /// `use` statement must start with to be flagged.
    pub(super) thiserror_crates: BTreeSet<Symbol>,
    /// Identifiers that, anywhere in the crate, name a configured
    /// path's terminal item. A bare `#[derive(X)]` where `X` is in
    /// this set is treated as thiserror-derived. Populated by the
    /// alias-collection visitor (`scan` submodule) from the
    /// `EarlyLintPass::check_crate` hook, before any `check_item`
    /// callback runs.
    pub(super) aliases: BTreeSet<Symbol>,
    /// Local names that alias a configured thiserror *crate*.
    /// Populated from `use thiserror as te;` and
    /// `extern crate thiserror as te;`. The value is the original
    /// crate name (e.g. `thiserror`) so that a derive path
    /// `[te, Error]` can be expanded to `[thiserror, Error]` and
    /// matched against `thiserror_paths`.
    pub(super) crate_aliases: BTreeMap<Symbol, Symbol>,
}

impl PreferDeriveMoreOverThiserror {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let configured = config.thiserror_paths.unwrap_or_else(|| {
            DEFAULT_THISERROR_PATHS
                .iter()
                .map(|path| (*path).to_owned())
                .collect()
        });
        let thiserror_paths: Vec<Vec<Symbol>> = configured
            .iter()
            .map(|path| {
                path.split("::")
                    .filter(|segment| !segment.is_empty())
                    .map(Symbol::intern)
                    .collect()
            })
            .filter(|segments: &Vec<Symbol>| !segments.is_empty())
            .collect();
        let thiserror_crates = thiserror_paths
            .iter()
            .filter_map(|segments| segments.first().copied())
            .collect();
        Self {
            thiserror_paths,
            thiserror_crates,
            aliases: BTreeSet::new(),
            crate_aliases: BTreeMap::new(),
        }
    }
}

/// Whether `path` (a sequence of bare segment symbols) exactly equals
/// one of the configured thiserror paths. Shared by the scan's
/// alias collection and the detection pass's derive matching.
pub(super) fn path_matches_thiserror(configured: &[Vec<Symbol>], path: &[Symbol]) -> bool {
    configured.iter().any(|cfg| cfg.as_slice() == path)
}
