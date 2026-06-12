//! Configuration for `import_grouping`: the chosen [`Style`], the
//! group [`order`](Config::order), the per-group classification lists
//! (`std_crates` / `internal_prefixes`), how `#[cfg(...)]`-gated
//! imports are slotted ([`CfgBlockHandling`]), and the exact blank-line
//! count that separates adjacent groups.

/// How `use` statements are partitioned into blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Style {
    /// Every `use` statement sits in one contiguous block, with no
    /// blank lines between imports.
    SingleBlock,
    /// Imports are partitioned into ordered groups separated by exactly
    /// `blank_line_count` blank lines. The default group set is
    /// std (`std` / `core` / `alloc`), internal (`super` / `self` /
    /// `crate`), and third-party (every other crate).
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
    /// Crate roots classified into the `std` group. Defaults to
    /// `["std", "core", "alloc"]`; extend with `proc_macro` or `test`
    /// if a project routinely imports them.
    #[serde(default = "default_std_crates")]
    pub(super) std_crates: Vec<String>,
    /// Path prefixes classified into the `internal` group. Defaults to
    /// `["crate", "super", "self"]`; extend with project-specific
    /// re-export roots treated as part of the workspace.
    #[serde(default = "default_internal_prefixes")]
    pub(super) internal_prefixes: Vec<String>,
    /// How `#[cfg(...)]`-gated imports are grouped. Defaults to
    /// `trailing`.
    #[serde(default)]
    pub(super) cfg_block_handling: CfgBlockHandling,
    /// Exact number of blank lines separating adjacent groups (strict
    /// equality). Defaults to `1`. Ignored under `single_block`.
    #[serde(default = "default_blank_line_count")]
    pub(super) blank_line_count: usize,
}

fn default_order() -> Vec<Group> {
    vec![Group::Std, Group::Internal, Group::Thirdparty]
}

fn default_std_crates() -> Vec<String> {
    ["std", "core", "alloc"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn default_internal_prefixes() -> Vec<String> {
    ["crate", "super", "self"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn default_blank_line_count() -> usize {
    1
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
    use super::{
        CfgBlockHandling, Config, Style, default_internal_prefixes, default_order,
        default_std_crates,
    };

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
        assert!(toml::from_str::<Config>("blank_line_count = 2").is_err());
    }

    #[test]
    fn other_fields_default_when_style_is_set() {
        // Only `style` is mandatory; the remaining knobs fall back to
        // their per-field defaults when absent.
        let config = toml::from_str::<Config>(r#"style = "multi_block""#).unwrap();
        assert_eq!(config.order, default_order());
        assert_eq!(config.std_crates, default_std_crates());
        assert_eq!(config.internal_prefixes, default_internal_prefixes());
        assert_eq!(config.cfg_block_handling, CfgBlockHandling::Trailing);
        assert_eq!(config.blank_line_count, 1);
    }

    #[test]
    fn unknown_style_is_rejected() {
        // There is no neutral `preserve` value; an unrecognised style is
        // a hard deserialisation error rather than a silent no-op.
        assert!(toml::from_str::<Config>(r#"style = "preserve""#).is_err());
    }
}
