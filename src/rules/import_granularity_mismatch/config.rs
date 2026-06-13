//! Configuration for `import_granularity_mismatch`: the chosen [`Style`], the
//! three `respect_*` knobs that decide which `use` statements may be
//! merged with one another, and the optional [`SelfMerge`] knob that
//! resolves the item-and-module name ambiguity under `crate` style.

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

/// How to resolve a path segment that names **both** an item and a
/// module under `crate` style — `use crate::thing;` next to
/// `use crate::thing::T;`. The two one-`use`-per-root shapes are not
/// interchangeable, so unless this knob is set the rule offers both and
/// the author picks. Only consulted under `style = "crate"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SelfMerge {
    /// Always enforce the `self`-fold `use crate::thing::{self, T};` —
    /// the shape rustfmt's `imports_granularity = "Crate"` produces. The
    /// sibling-split form is flagged and rewritten to it.
    Fold,
    /// Always enforce the sibling-split `use crate::{thing, thing::T};`.
    /// The `self`-fold form is flagged and rewritten to it.
    Split,
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
    /// Under `style = "crate"`, force one shape when a path segment
    /// names both an item and a module (`use crate::thing;` next to
    /// `use crate::thing::T;`). Unset by default — the two shapes are
    /// not interchangeable, so the rule flags neither single-statement
    /// form and offers both as `MaybeIncorrect` when a merge is forced.
    /// Set `fold` to always enforce `use crate::thing::{self, T};`, or
    /// `split` to always enforce `use crate::{thing, thing::T};`;
    /// either value also flags the opposite single-statement form.
    /// Ignored under `module` and `item` style.
    pub(super) self_merge: Option<SelfMerge>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Style::default(),
            respect_cfg_blocks: true,
            respect_visibility: true,
            respect_doc_comments: true,
            self_merge: None,
        }
    }
}
