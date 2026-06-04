//! Configuration and in-memory state for
//! `prefer_derive_more_over_thiserror`: the empty [`Config`] shape, the
//! recognised `thiserror` paths, the [`PreferDeriveMoreOverThiserror`]
//! pass state, and the path matcher shared by the
//! [`scan`](super::scan) and [`detect`](super::detect) submodules.

use std::collections::{BTreeMap, BTreeSet};

use rustc_span::Symbol;

const CONFIG_KEY: &str = "perfectionist::prefer_derive_more_over_thiserror";

/// The `thiserror` derive paths the rule recognises.
const THISERROR_PATHS: &[&str] = &["thiserror::Error"];

/// Configuration is reserved for future knobs; the lint currently
/// has no options. The empty struct still exists so that a stray
/// `[perfectionist::prefer_derive_more_over_thiserror]` table in
/// `dylint.toml` deserialises rather than producing a confusing
/// parse error.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

/// The recognised thiserror paths plus the crate-wide alias maps the
/// pass accumulates in its pre-expansion `check_crate` scan. Fields are
/// `pub(super)` so the sibling `scan` and `detect` submodules (both
/// descendants of the rule's entry module) can read and, in the
/// scan's case, populate them.
pub(super) struct PreferDeriveMoreOverThiserror {
    /// Recognised paths split into segment lists (e.g.
    /// `[[thiserror, Error]]`).
    pub(super) thiserror_paths: Vec<Vec<Symbol>>,
    /// First segments of every recognised path — the crate names a
    /// `use` statement must start with to be flagged.
    pub(super) thiserror_crates: BTreeSet<Symbol>,
    /// Identifiers that, anywhere in the crate, name a recognised
    /// path's terminal item. A bare `#[derive(X)]` where `X` is in
    /// this set is treated as thiserror-derived. Populated by the
    /// alias-collection visitor (`scan` submodule) from the
    /// `EarlyLintPass::check_crate` hook, before any `check_item`
    /// callback runs.
    pub(super) aliases: BTreeSet<Symbol>,
    /// Local names that alias a recognised thiserror *crate*.
    /// Populated from `use thiserror as te;` and
    /// `extern crate thiserror as te;`. The value is the original
    /// crate name (e.g. `thiserror`) so that a derive path
    /// `[te, Error]` can be expanded to `[thiserror, Error]` and
    /// matched against `thiserror_paths`.
    pub(super) crate_aliases: BTreeMap<Symbol, Symbol>,
}

impl PreferDeriveMoreOverThiserror {
    pub(super) fn new() -> Self {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let thiserror_paths: Vec<Vec<Symbol>> = THISERROR_PATHS
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
/// one of the recognised thiserror paths. Shared by the scan's
/// alias collection and the detection pass's derive matching.
pub(super) fn path_matches_thiserror(recognised: &[Vec<Symbol>], path: &[Symbol]) -> bool {
    recognised.iter().any(|known| known.as_slice() == path)
}
