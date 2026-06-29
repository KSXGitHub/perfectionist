//! Configuration for `import_grouping_mismatch`: the chosen [`Style`], the
//! group [`order`](Config::order), how `#[cfg(...)]`-gated imports are
//! slotted ([`CfgBlockHandling`]), and how `pub` re-exports are grouped
//! ([`reexports`](Config::reexports) / [`ReexportGrouping`]).

/// How `use` statements are partitioned into blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Style {
    /// Every `use` statement sits in one contiguous block, with no
    /// blank lines between imports.
    SingleBlock,
    /// Imports are partitioned into ordered groups separated by exactly
    /// one blank line. The group set is
    /// std (`std` / `core` / `alloc` / `proc_macro` / `test`), internal
    /// (`crate` / `super` / `self`), and third-party (every other crate).
    MultiBlock,
}

/// One of the three groups a `use` statement is classified into. The
/// `order` knob is a permutation of these three values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Group {
    /// `std`, `core`, `alloc`, `proc_macro`, `test`.
    Std,
    /// `crate`, `super`, `self`.
    Internal,
    /// Every other crate.
    Thirdparty,
}

/// How `pub` re-exports are grouped relative to the private imports. A
/// re-export is any `use` with an explicit visibility (`pub`,
/// `pub(crate)`, `pub(super)`, `pub(in ...)`); a private (`Inherited`)
/// import is not one.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReexportGrouping {
    /// Re-exports get no dedicated block: each is classified purely by
    /// its path, exactly like a private import, so a `pub use child::Item`
    /// sits in the same block as a private import of the same origin.
    ByPath,
    /// Every re-export is pulled into one contiguous leading block above
    /// all private imports, separated by a blank line. A cfg-gated
    /// re-export stays in this block rather than the trailing cfg block:
    /// visibility takes precedence, keeping the public surface together.
    #[default]
    Grouped,
    /// Re-exports form a leading region split into two blank-separated
    /// blocks: *submodule* re-exports (a multi-segment path such as
    /// `pub use child::Item;`, which lifts an item out of a child module)
    /// above *alias* re-exports (a single-segment path such as
    /// `pub use Item;` or `pub use Item as Alias;`, which only renames an
    /// item already in scope). The single-vs-multi-segment split is
    /// purely syntactic; a `pub use foo;` that re-exports an external
    /// crate counts as an alias re-export. As under `grouped`, a cfg-gated
    /// re-export stays in its re-export sub-block rather than the trailing
    /// cfg block.
    Split,
}

/// How a `#[cfg(...)]`-gated import is grouped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CfgBlockHandling {
    /// Give every `#[cfg(...)]`-gated import its own trailing block,
    /// regardless of the imported path: an always-last group under
    /// `multi_block`, a trailing block below the single block under
    /// `single_block`.
    #[default]
    Trailing,
    /// Keep a cfg-gated import with the rest: slotted into its natural
    /// path group under `multi_block`, or left in the single block under
    /// `single_block`.
    Merge,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    // A bare `Style` (not `Option<Style>`) with no `serde(default)`, so
    // `style` is a required field: an enabled rule with no `style` fails
    // to deserialize rather than silently defaulting to a layout. This
    // is also the syntactic signal gen-docs reads to badge the field
    // `mandatory`. Every other field keeps a per-field `serde(default)`
    // so only `style` is mandatory; the config is read only when the
    // rule is enabled (see `register_pass`), so a disabled rule never
    // needs it.
    /// The grouping style to enforce: `single_block` or `multi_block`. It
    /// has no default — a project enabling the rule states which layout
    /// it wants — so it must be set when the rule is enabled.
    pub(super) style: Style,
    /// The order the groups appear in, top to bottom. Defaults to
    /// `["std", "internal", "thirdparty"]`.
    #[serde(default = "default_order")]
    pub(super) order: Vec<Group>,
    /// How `#[cfg(...)]`-gated imports are grouped. Defaults to
    /// `trailing`: a cfg-gated import forms its own trailing block under
    /// both styles. Set `merge` to keep cfg-gated imports with the rest —
    /// in their natural path group under `multi_block`, or in the single
    /// block under `single_block`.
    #[serde(default)]
    pub(super) cfg_block_handling: CfgBlockHandling,
    /// How `pub` re-exports are grouped. Defaults to `grouped`: every
    /// re-export is pulled into one contiguous leading block above all
    /// private imports, separated by a blank line. Set `split` to break
    /// that leading block into two — submodule re-exports
    /// (`pub use child::Item;`) above alias re-exports (`pub use Item;` /
    /// `pub use Item as Alias;`) — or `by_path` to give re-exports no
    /// dedicated block at all, classifying each by its path like a private
    /// import.
    #[serde(default)]
    pub(super) reexports: ReexportGrouping,
}

fn default_order() -> Vec<Group> {
    vec![Group::Std, Group::Internal, Group::Thirdparty]
}

impl Config {
    /// How far the private-import ranks are shifted down to make room for
    /// the leading re-export region: `0` when re-exports are classified
    /// `by_path` (no region), `1` for the single `grouped` block, `2` for
    /// the two `split` blocks (submodule then alias). Added to every
    /// non-re-export rank in [`super::classify::rank`].
    pub(super) fn reexport_block_offset(&self) -> usize {
        match self.reexports {
            ReexportGrouping::ByPath => 0,
            ReexportGrouping::Grouped => 1,
            ReexportGrouping::Split => 2,
        }
    }

    /// Whether re-exports are pulled into their own leading region rather
    /// than classified by path — true for `grouped` and `split`, false
    /// for `by_path`. A re-export in such a region outranks its cfg gating,
    /// so it never joins the trailing cfg block.
    pub(super) fn separates_reexports(&self) -> bool {
        !matches!(self.reexports, ReexportGrouping::ByPath)
    }

    /// Rank of the `std` / `internal` / `thirdparty` group `group`
    /// within the configured `order` — its zero-based position, used to
    /// compare two statements' groups. A group absent from `order`
    /// (only possible with a partial user-supplied list) sorts after
    /// every listed group but before the trailing cfg group.
    pub(super) fn group_rank(&self, group: Group) -> usize {
        self.order
            .iter()
            .position(|listed| *listed == group)
            .unwrap_or(self.order.len())
    }

    /// Rank of the always-last cfg group under
    /// [`CfgBlockHandling::Trailing`]: strictly after every path-based
    /// group, including one absent from `order`.
    pub(super) fn cfg_rank(&self) -> usize {
        self.order.len() + 1
    }
}

#[cfg(test)]
mod tests;
