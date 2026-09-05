//! Tests for `overly_complex_condition`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/overly_complex_condition.rs`
//! and is picked up by `tests/ui.rs`. The `max_operators` knob is
//! covered by a UI fixture under `ui-toml/overly_complex_condition/` run
//! with a per-rule `dylint.toml`; `test_code_exception` needs
//! `#[cfg(test)]` code to exist, so it is covered by a minimal Cargo
//! project run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::overly_complex_condition";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_operators: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_every_operator_count() {
    let fixtures = _utils::copy_fixtures_with_directive(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/overly_complex_condition/zero_threshold",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig {
            max_operators: Some(0),
        }))
        .run();
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each hold a condition of four operators — one
/// above the default limit. The conditions are the only ones in the
/// fixture, so a flag is identified by the line it points at.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/overly_complex_condition/lib_with_test_module.rs");

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
    let stderr = run("fixture_occ_default", "");
    assert_flagged(&stderr, "src/lib.rs:2:8");
    assert_flagged(&stderr, "src/lib.rs:8:12");
    assert_flagged(&stderr, "src/lib.rs:14:12");
}

#[test]
fn test_code_exception_leaves_test_code_alone() {
    let stderr = run(
        "fixture_occ_test_exception",
        text_block_fnl! {
            r#"["perfectionist::overly_complex_condition"]"#
            "test_code_exception = true"
        },
    );
    assert_flagged(&stderr, "src/lib.rs:2:8");
    assert_not_flagged(&stderr, "src/lib.rs:8:12");
    assert_not_flagged(&stderr, "src/lib.rs:14:12");
}
