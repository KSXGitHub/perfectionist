//! UI tests for `import_grouping`'s configuration knobs. The
//! default-config (`style = "grouped"`) sweep lives in
//! `ui/import_grouping.rs` and is picked up by `tests/ui.rs`; these
//! tests each point at their own one-fixture directory under
//! `ui-toml/import_grouping/` and pass a per-rule `dylint.toml` to
//! [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared `Mutex`
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::import_grouping";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept separate from the lint's own internal `Config`
/// so the test surface is independent of the implementation's private
/// struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    std_crates: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    internal_prefixes: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cfg_block_handling: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blank_line_count: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // The rule is active by default, so unlike the opt-in rules' test
    // harnesses no `[perfectionist] enable = [...]` table is needed —
    // only the per-rule `[perfectionist::import_grouping]` knobs.
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
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
fn single_group_collapses_blank_lines() {
    // Under `style = "single_group"`, every blank line between imports
    // is flagged and the block is re-rendered as one contiguous run in
    // source order.
    run(
        "ui-toml/import_grouping/single_group",
        RuleConfig {
            style: Some("single_group"),
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
        "ui-toml/import_grouping/custom_order",
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
        "ui-toml/import_grouping/cfg_merge",
        RuleConfig {
            cfg_block_handling: Some("merge"),
            ..Default::default()
        },
    );
}

#[test]
fn blank_line_count_two_separates_groups() {
    // `blank_line_count = 2` requires exactly two blank lines between
    // groups; a single blank line is a violation.
    run(
        "ui-toml/import_grouping/blank_line_count",
        RuleConfig {
            blank_line_count: Some(2),
            ..Default::default()
        },
    );
}

#[test]
fn internal_prefixes_extends_workspace_root() {
    // Adding a workspace crate to `internal_prefixes` groups its
    // imports with `crate` / `super` / `self`, ahead of third-party.
    run(
        "ui-toml/import_grouping/internal_prefixes",
        RuleConfig {
            internal_prefixes: Some(vec!["crate", "super", "self", "my_macros"]),
            ..Default::default()
        },
    );
}
