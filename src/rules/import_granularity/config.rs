//! Configuration for `import_granularity`: the chosen [`Style`] plus
//! the three `respect_*` knobs that decide which `use` statements may
//! be merged with one another.

/// Import-granularity style. The three values map one-to-one onto
/// rustfmt's unstable `imports_granularity` option (`Crate`, `Module`,
/// `Item`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Style {
    /// One `use` per crate root. Every shared prefix is collapsed into
    /// nested braces, e.g.
    /// `use std::{collections::HashMap, io::{Error, ErrorKind}};`.
    Crate,
    /// One `use` per leaf module. Items pulled from the same module are
    /// merged into a single braced list; items from sibling modules sit
    /// on their own `use` lines, e.g.
    /// `use std::collections::{BTreeMap, HashMap};`.
    #[default]
    Module,
    /// One `use` per leaf item. Every imported name lives on its own
    /// line, e.g. `use std::collections::BTreeMap;`.
    Item,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Import-granularity style to enforce. Defaults to `module` — the
    /// shape that scales best as a `use` block grows. Set `crate` to
    /// collapse every crate root into one nested `use`, or `item` to
    /// put every imported name on its own line.
    pub(super) style: Style,
    /// Never merge `use` statements that carry differing `#[cfg(...)]`
    /// / `#[cfg_attr(...)]` attributes. Defaults to `true`: a
    /// platform-gated import is never folded together with an
    /// unconditional one. Set `false` to ignore cfg attributes when
    /// deciding what may merge.
    pub(super) respect_cfg_blocks: bool,
    /// Never merge a `pub use` (or `pub(crate) use`, etc.) with a
    /// plain `use`, or two re-exports whose visibility differs.
    /// Defaults to `true`. Set `false` to ignore visibility when
    /// deciding what may merge.
    pub(super) respect_visibility: bool,
    /// Never merge a `use` that carries its own doc comment (`///` or
    /// `#[doc = "..."]`) into a neighbouring statement, so the comment
    /// keeps describing exactly the import it was written above.
    /// Defaults to `true`. Set `false` to allow such a `use` to merge.
    pub(super) respect_doc_comments: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Style::default(),
            respect_cfg_blocks: true,
            respect_visibility: true,
            respect_doc_comments: true,
        }
    }
}
