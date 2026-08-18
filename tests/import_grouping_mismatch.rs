//! UI tests for `import_grouping_mismatch`'s configuration knobs. The rule is
//! inactive by default and `style` is mandatory once enabled, so each
//! test points at its own one-fixture directory under
//! `ui-toml/import_grouping_mismatch/` and passes a `dylint.toml` that both
//! enables the rule and selects a style. The bare-`multi_block` sweep
//! lives in `ui-toml/import_grouping_mismatch/multi_block/` and has its own test below.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::import_grouping_mismatch";

static SERIAL: Mutex<()> = Mutex::new(());

/// The subset of the rule's user-facing configuration these tests
/// exercise, mirrored here for serialisation. Kept separate from the
/// lint's own internal `Config` so the test surface is independent of
/// the implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cfg_block_handling: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reexports: Option<&'static str>,
}

fn dylint_toml(mut config: RuleConfig) -> String {
    // The rule is inactive by default, so the config must enable it.
    // `style` and `reexports` are both mandatory once enabled; the
    // knob-focused tests below leave them unset, meaning the default
    // `multi_block` / `grouped` layout, so fill them in here rather than
    // at every call site.
    config.style.get_or_insert("multi_block");
    config.reexports.get_or_insert("grouped");
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    let rule_table = toml::to_string(&table).expect("serialise rule config as dylint.toml");
    format!("[perfectionist]\nenable = [\"import_grouping_mismatch\"]\n\n{rule_table}")
}

fn run(src_base: &str, config: RuleConfig) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn single_block_collapses_blank_lines() {
    // Under `style = "single_block"`, every blank line between imports
    // is flagged and the block is re-rendered as one contiguous run in
    // source order.
    run(
        "ui-toml/import_grouping_mismatch/single_block",
        RuleConfig {
            style: Some("single_block"),
            ..Default::default()
        },
    );
}

#[test]
fn single_block_separates_cfg_into_trailing_block_by_default() {
    // Under `style = "single_block"`, `cfg_block_handling` defaults to
    // `trailing`: the non-cfg imports stay in one contiguous block and
    // every `#[cfg(...)]`-gated import is hoisted into a single trailing
    // block one blank line below. No cfg knob is set, so this also
    // asserts the default.
    run(
        "ui-toml/import_grouping_mismatch/single_block_cfg_trailing",
        RuleConfig {
            style: Some("single_block"),
            ..Default::default()
        },
    );
}

#[test]
fn single_block_cfg_merge_keeps_cfg_in_one_block() {
    // Under `style = "single_block"` with `cfg_block_handling = "merge"`,
    // cfg-gated imports are not separated: a blank line above one is a
    // violation, and the fix collapses everything into one block.
    run(
        "ui-toml/import_grouping_mismatch/single_block_cfg_merge",
        RuleConfig {
            style: Some("single_block"),
            cfg_block_handling: Some("merge"),
            ..Default::default()
        },
    );
}

#[test]
fn std_group_includes_proc_macro_and_test() {
    // The std group is the fixed set `std` / `core` / `alloc` /
    // `proc_macro` / `test`. A blank line between a `std` import and a
    // `proc_macro` / `test` import wrongly splits one group, so it is a
    // violation the fix collapses into one contiguous block.
    run(
        "ui-toml/import_grouping_mismatch/std_builtin_crates",
        RuleConfig {
            ..Default::default()
        },
    );
}

#[test]
fn custom_order_thirdparty_before_internal() {
    // `order = ["std", "thirdparty", "internal"]` (rustfmt's
    // `StdExternalCrate` shape) puts third-party crates before internal
    // imports; the default-order layout is now a violation.
    run(
        "ui-toml/import_grouping_mismatch/custom_order",
        RuleConfig {
            order: Some(vec!["std", "thirdparty", "internal"]),
            ..Default::default()
        },
    );
}

#[test]
fn cfg_merge_slots_by_path() {
    // `cfg_block_handling = "merge"` classifies a cfg-gated import by
    // its path instead of hoisting it to a trailing group, so a
    // cfg-gated std import belongs in the std group.
    run(
        "ui-toml/import_grouping_mismatch/cfg_merge",
        RuleConfig {
            cfg_block_handling: Some("merge"),
            ..Default::default()
        },
    );
}

#[test]
fn bare_path_local_submodule_is_internal() {
    // Regression: a bare-path import of a first-party submodule (`use
    // error::Foo;` where `error` is a sibling `mod`) is grouped with the
    // internal block, ahead of third-party, instead of being treated as
    // a third-party crate keyed on the bare first segment.
    run(
        "ui-toml/import_grouping_mismatch/local_submodule",
        RuleConfig::default(),
    );
}

#[test]
fn multi_block_grouped_reexports_lead_in_their_own_block() {
    // Under `multi_block` with the default `reexports = "grouped"`, every
    // `pub` re-export is pulled into one leading block above the path-
    // partitioned private imports. Visibility outranks path and cfg
    // gating, so a `pub use std::...` and a cfg-gated `pub use` both join
    // the leading block rather than their natural std / trailing cfg
    // group. The knob is set explicitly to document the intent even
    // though `grouped` is the default.
    run(
        "ui-toml/import_grouping_mismatch/multi_block_separate_reexports",
        RuleConfig {
            reexports: Some("grouped"),
            ..Default::default()
        },
    );
}

#[test]
fn single_block_grouped_reexports_lead_in_their_own_block() {
    // Under `single_block` with `reexports = "grouped"`, `pub` re-exports
    // form a leading block, the private imports collapse into one block
    // below, and a cfg-gated private import still forms a trailing block.
    run(
        "ui-toml/import_grouping_mismatch/single_block_separate_reexports",
        RuleConfig {
            style: Some("single_block"),
            reexports: Some("grouped"),
            ..Default::default()
        },
    );
}

#[test]
fn multi_block_split_reexports_form_two_leading_blocks() {
    // Under `multi_block` with `reexports = "split"`, the leading
    // re-export region is broken into two blocks: submodule re-exports
    // (a `::`-qualified path) above alias re-exports (a single-segment
    // path), each blank-separated, both above the path-partitioned
    // private imports.
    run(
        "ui-toml/import_grouping_mismatch/multi_block_split_reexports",
        RuleConfig {
            reexports: Some("split"),
            ..Default::default()
        },
    );
}

#[test]
fn single_block_split_reexports_form_two_leading_blocks() {
    // Under `single_block` with `reexports = "split"`, submodule
    // re-exports and alias re-exports form two leading blocks, the
    // private imports collapse into one block below, and a cfg-gated
    // private import still forms a trailing block. A cfg-gated alias
    // re-export stays in the alias sub-block — visibility/kind outranks
    // cfg gating.
    run(
        "ui-toml/import_grouping_mismatch/single_block_split_reexports",
        RuleConfig {
            style: Some("single_block"),
            reexports: Some("split"),
            ..Default::default()
        },
    );
}

#[test]
fn multi_block_reexports_classified_by_path_when_disabled() {
    // `reexports = "by_path"` (the non-default) turns off the leading
    // re-export block: a `pub use crate::...` is classified internal by
    // its path and groups with the private `use crate::...` import instead
    // of leading in its own block.
    run(
        "ui-toml/import_grouping_mismatch/multi_block_reexports_by_path",
        RuleConfig {
            reexports: Some("by_path"),
            ..Default::default()
        },
    );
}

#[test]
fn multi_block_partitions_into_ordered_blocks() {
    // The bare-default `multi_block` style with no knobs overridden:
    // imports must be partitioned into std / internal / third-party
    // blocks separated by one blank line. This is the broad sweep that
    // ran via `tests/ui.rs` while the rule was still active by default.
    run(
        "ui-toml/import_grouping_mismatch/multi_block",
        RuleConfig::default(),
    );
}
