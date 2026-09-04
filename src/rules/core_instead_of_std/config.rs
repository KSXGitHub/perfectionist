//! Configuration for `core_instead_of_std`: whether `alloc::` paths
//! count alongside `core::` ones, and the paths that are never flagged.

use std::collections::BTreeSet;

/// The user-facing configuration shape, deserialised from the
/// `["perfectionist::core_instead_of_std"]` table of `dylint.toml`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Whether an `alloc::` path is a violation alongside a `core::`
    /// one. Set it to `false` in a crate that keeps its `alloc::` paths
    /// on purpose — one tracking `core` cleanliness against a possible
    /// `no_std` future while the `alloc` dependency is permanent.
    /// Defaults to `true`.
    pub(super) also_alloc: bool,
    /// Paths that are never flagged, each written as the absolute
    /// extern-crate path it is — with the leading `::` and every
    /// segment spelled out, as in `"::core::mem::transmute"`. Only a
    /// `::core::` or `::alloc::` entry can ever match, and matching is
    /// exact and syntactic against the path as written, with no
    /// re-export or alias resolution. Exempting a path also withdraws
    /// the automatic rewrite from any name sharing its crate segment
    /// (`use core::{mem::transmute, fmt::Display};`), because that
    /// rewrite would move the exempted path too. Defaults to `[]`.
    pub(super) skip_paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            also_alloc: true,
            skip_paths: Vec::new(),
        }
    }
}

/// The resolved, lookup-ready form of [`Config`] held by the running
/// pass: the exemption list interned into a set for membership tests.
pub(super) struct Resolved {
    pub(super) also_alloc: bool,
    pub(super) skip_paths: BTreeSet<String>,
}

impl Resolved {
    pub(super) fn from_config(config: Config) -> Self {
        Self {
            also_alloc: config.also_alloc,
            skip_paths: config.skip_paths.into_iter().collect(),
        }
    }
}

/// Validate that every `skip_paths` entry is a `::core::` / `::alloc::`
/// path with at least one segment under the crate. The rule only ever
/// flags such a path, so an entry of any other shape exempts nothing and
/// is almost certainly a mistake — a bare `core::fmt::Display` missing
/// its leading `::`, or a path into some other crate. Returns an error
/// message — for the dylint run to fail on — naming the first offending
/// entry.
pub(super) fn validate(config: &Config) -> Result<(), String> {
    for entry in &config.skip_paths {
        let mut segments = entry.split("::");
        let rooted = segments.next() == Some("");
        let crate_name = segments.next();
        let under_crate: Vec<&str> = segments.collect();
        if !rooted
            || !matches!(crate_name, Some("core" | "alloc"))
            || under_crate.is_empty()
            || under_crate.iter().any(|segment| segment.trim().is_empty())
        {
            return Err(format!(
                "`skip_paths` entry {entry:?} is not a path this rule could flag: \
                 write the whole path as an absolute extern-crate path into `core` \
                 or `alloc`, with the leading `::` (e.g. `::core::mem::transmute`)",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
