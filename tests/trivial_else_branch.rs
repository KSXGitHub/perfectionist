//! Tests for `trivial_else_branch`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/trivial_else_branch.rs`
//! and is picked up by `tests/ui.rs`. The `min_then_statements` knob is
//! covered by a UI fixture under `ui-toml/trivial_else_branch/` run
//! with a per-rule `dylint.toml`; `test_code_exception` needs
//! `#[cfg(test)]` code to exist, so it is covered by a minimal Cargo
//! project run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::trivial_else_branch";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    min_then_statements: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_every_trivial_else() {
    dylint_testing::ui::Test::src_base(
        env!("CARGO_PKG_NAME"),
        "ui-toml/trivial_else_branch/zero_threshold",
    )
    .dylint_toml(dylint_toml(RuleConfig {
        min_then_statements: Some(0),
    }))
    .run();
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each hold an upside-down guard clause. The
/// `else` branches are the only ones in the fixture, so a flag is
/// identified by the line it points at.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/trivial_else_branch/lib_with_test_module.rs");

/// Run the fixture and return its stderr, asserting that `cargo dylint`
/// itself succeeded.
fn run(package_name: &str, config: &str) -> String {
    let (_temp, stderr, success) = run_project_with_config(
        package_name,
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", LIB_WITH_TEST_MODULE)],
        config,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

fn assert_flagged(stderr: &str, function: &str) {
    assert!(
        stderr.contains(function),
        "expected `{function}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, function: &str) {
    assert!(
        !stderr.contains(function),
        "expected `{function}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_teb_default", "");
    assert_flagged(&stderr, "src/lib.rs:6:12");
    assert_flagged(&stderr, "src/lib.rs:18:16");
    assert_flagged(&stderr, "src/lib.rs:30:16");
}

#[test]
fn test_code_exception_leaves_test_code_alone() {
    let stderr = run(
        "fixture_teb_test_exception",
        text_block_fnl! {
            r#"["perfectionist::trivial_else_branch"]"#
            "test_code_exception = true"
        },
    );
    assert_flagged(&stderr, "src/lib.rs:6:12");
    assert_not_flagged(&stderr, "src/lib.rs:18:16");
    assert_not_flagged(&stderr, "src/lib.rs:30:16");
}
