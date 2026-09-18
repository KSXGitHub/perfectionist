//! UI tests for `import_granularity_mismatch`'s configuration knobs. The
//! default-config (`style = "module"`) sweep lives in
//! `ui/import_granularity_mismatch.rs` and is picked up by `tests/ui.rs`; these
//! tests each point at their own one-fixture directory under
//! `ui-toml/import_granularity_mismatch/` and pass a per-rule `dylint.toml` to
//! `_utils::configured_ui_test`.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::import_granularity_mismatch";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    respect_cfg_blocks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    respect_visibility: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    respect_doc_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    self_merge: Option<&'static str>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // The rule is active by default, so unlike the opt-in rules'
    // test harnesses no `[perfectionist] enable = [...]` table is
    // needed — only the per-rule knobs under
    // `["perfectionist::import_granularity_mismatch"]`.
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    let rule_table = toml::to_string(&table).expect("serialise rule config as dylint.toml");
    // The fixtures order their support modules, imports and re-exports
    // to read as cases for the rule under test, a layout
    // `arbitrary_source_item_ordering` flags; disable it so its findings
    // stay out of the snapshot.
    format!(
        "[perfectionist]\n\
         disable = [\"arbitrary_source_item_ordering\"]\n\
         \n\
         {rule_table}",
    )
}

fn run(src_base: &str, config: RuleConfig) {
    _utils::configured_ui_test(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_MANIFEST_DIR"),
        src_base,
        dylint_toml(config),
    )
    .run();
}

#[test]
fn crate_style_collapses_per_root() {
    run(
        "ui-toml/import_granularity_mismatch/crate_style",
        RuleConfig {
            style: Some("crate"),
            ..Default::default()
        },
    );
}

#[test]
fn item_style_splits_per_leaf() {
    run(
        "ui-toml/import_granularity_mismatch/item_style",
        RuleConfig {
            style: Some("item"),
            ..Default::default()
        },
    );
}

#[test]
fn self_merge_fold_enforces_self() {
    // `crate` style with `self_merge = "fold"`: a name that is both an
    // item and a module is always written `crate::thing::{self, T}`. The
    // sibling-split single statement is flagged and rewritten to it.
    run(
        "ui-toml/import_granularity_mismatch/self_merge_fold",
        RuleConfig {
            style: Some("crate"),
            self_merge: Some("fold"),
            ..Default::default()
        },
    );
}

#[test]
fn self_merge_split_enforces_siblings() {
    // `crate` style with `self_merge = "split"`: the same name is always
    // written `crate::{thing, thing::T}`. The `self`-fold single
    // statement is flagged and rewritten to it.
    run(
        "ui-toml/import_granularity_mismatch/self_merge_split",
        RuleConfig {
            style: Some("crate"),
            self_merge: Some("split"),
            ..Default::default()
        },
    );
}

#[test]
fn respect_doc_comments_false_allows_merge() {
    // Default `module` style, but the doc-commented `use` is now
    // allowed to merge with its plain same-module neighbour.
    run(
        "ui-toml/import_granularity_mismatch/respect_doc_comments",
        RuleConfig {
            respect_doc_comments: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn respect_visibility_false_flags_without_fixing() {
    // With visibility ignored for grouping, a `pub use` and a private
    // `use` from the same module are flagged together — but the fix is
    // withheld because merging can't preserve both visibilities.
    run(
        "ui-toml/import_granularity_mismatch/respect_visibility",
        RuleConfig {
            respect_visibility: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn respect_cfg_blocks_false_flags_without_fixing() {
    // With cfg gates ignored for grouping, a platform-gated `use` and an
    // unconditional one from the same module are flagged together — but
    // the fix is withheld because merging would drop the `#[cfg]` gate.
    run(
        "ui-toml/import_granularity_mismatch/respect_cfg_blocks",
        RuleConfig {
            respect_cfg_blocks: Some(false),
            ..Default::default()
        },
    );
}
