//! Configuration for `import_grouping`: the chosen [`Style`], the
//! group [`order`](Config::order), the per-group classification lists
//! (`std_crates` / `internal_prefixes`), how `#[cfg(...)]`-gated
//! imports are slotted ([`CfgBlockHandling`]), and the exact blank-line
//! count that separates adjacent groups.

/// How `use` statements are partitioned into blocks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Style {
    /// Every `use` statement sits in one contiguous block, with no
    /// blank lines between imports.
    SingleBlock,
    /// Imports are partitioned into ordered groups separated by exactly
    /// `blank_line_count` blank lines. The default group set is
    /// std (`std` / `core` / `alloc`), internal (`super` / `self` /
    /// `crate`), and third-party (every other crate).
    #[default]
    MultiBlock,
}

/// One of the three groups a `use` statement is classified into. The
/// `order` knob is a permutation of these three values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Group {
    /// `std`, `core`, `alloc` (configurable via `std_crates`).
    Std,
    /// `super`, `self`, `crate` (configurable via `internal_prefixes`).
    Internal,
    /// Every other crate.
    Thirdparty,
}

/// How a `#[cfg(...)]`-gated import is grouped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CfgBlockHandling {
    /// Treat every `#[cfg(...)]`-gated import as a fourth,
    /// always-last group, regardless of the imported path.
    #[default]
    Trailing,
    /// Slot a cfg-gated import back into its natural group based on the
    /// imported path's first segment.
    Merge,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Partitioning style to enforce. Defaults to `multi_block`.
    pub(super) style: Style,
    /// The order the groups appear in, top to bottom. Defaults to
    /// `["std", "internal", "thirdparty"]`.
    pub(super) order: Vec<Group>,
    /// Crate roots classified into the `std` group. Defaults to
    /// `["std", "core", "alloc"]`; extend with `proc_macro` or `test`
    /// if a project routinely imports them.
    pub(super) std_crates: Vec<String>,
    /// Path prefixes classified into the `internal` group. Defaults to
    /// `["crate", "super", "self"]`; extend with project-specific
    /// re-export roots treated as part of the workspace.
    pub(super) internal_prefixes: Vec<String>,
    /// How `#[cfg(...)]`-gated imports are grouped. Defaults to
    /// `trailing`.
    pub(super) cfg_block_handling: CfgBlockHandling,
    /// Exact number of blank lines separating adjacent groups (strict
    /// equality). Defaults to `1`. Ignored under `single_block`.
    pub(super) blank_line_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Style::default(),
            order: vec![Group::Std, Group::Internal, Group::Thirdparty],
            std_crates: ["std", "core", "alloc"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            internal_prefixes: ["crate", "super", "self"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            cfg_block_handling: CfgBlockHandling::default(),
            blank_line_count: 1,
        }
    }
}

impl Config {
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
