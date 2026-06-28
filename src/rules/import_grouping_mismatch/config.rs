//! Configuration for `import_grouping_mismatch`: the chosen [`Style`], the
//! group [`order`](Config::order), how `#[cfg(...)]`-gated imports are
//! slotted ([`CfgBlockHandling`]), and whether `pub` re-exports form their
//! own leading block
//! ([`separate_reexports`](Config::separate_reexports)).

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
    /// Whether `pub` re-exports form their own leading block. Defaults to
    /// `true`: every re-export — any `use` with an explicit visibility
    /// (`pub`, `pub(crate)`, `pub(super)`, `pub(in ...)`) — is pulled into
    /// one contiguous block above all private imports, separated by a
    /// blank line. A cfg-gated re-export stays in the re-export block
    /// rather than the trailing cfg block: visibility takes precedence,
    /// keeping the public surface together. Set `false` to classify a
    /// `use` purely by its path instead, so a `pub use child::Item` sits
    /// in the same block as a private import of the same origin.
    #[serde(default = "default_separate_reexports")]
    pub(super) separate_reexports: bool,
}

fn default_order() -> Vec<Group> {
    vec![Group::Std, Group::Internal, Group::Thirdparty]
}

fn default_separate_reexports() -> bool {
    true
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

#[cfg(test)]
mod tests {
    use super::{CfgBlockHandling, Config, Style, default_order};

    #[test]
    fn style_values_deserialize() {
        assert_eq!(
            toml::from_str::<Config>(r#"style = "multi_block""#)
                .unwrap()
                .style,
            Style::MultiBlock,
        );
        assert_eq!(
            toml::from_str::<Config>(r#"style = "single_block""#)
                .unwrap()
                .style,
            Style::SingleBlock,
        );
    }

    #[test]
    fn missing_style_is_an_error() {
        // `style` is required (bare `Style`, no `serde(default)`), so a
        // table that omits it fails to deserialize rather than defaulting
        // to a layout — even when another knob is present.
        assert!(toml::from_str::<Config>("").is_err());
        assert!(toml::from_str::<Config>(r#"order = ["std"]"#).is_err());
    }

    #[test]
    fn other_fields_default_when_style_is_set() {
        // Only `style` is mandatory; the remaining knobs fall back to
        // their per-field defaults when absent.
        let config = toml::from_str::<Config>(r#"style = "multi_block""#).unwrap();
        assert_eq!(config.order, default_order());
        assert_eq!(config.cfg_block_handling, CfgBlockHandling::Trailing);
        assert!(config.separate_reexports);
    }

    #[test]
    fn unknown_style_is_rejected() {
        // There is no neutral `preserve` value; an unrecognised style is
        // a hard deserialisation error rather than a silent no-op.
        assert!(toml::from_str::<Config>(r#"style = "preserve""#).is_err());
    }
}
