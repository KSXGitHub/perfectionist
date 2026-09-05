//! Tests for `too_many_struct_fields`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/too_many_struct_fields.rs`
//! and is picked up by `tests/ui.rs`. The `max_fields` knob is
//! covered by a UI fixture under `ui-toml/too_many_struct_fields/` run
//! with a per-rule `dylint.toml`; `test_code_exception` needs
//! `#[cfg(test)]` code to exist, so it is covered by a minimal Cargo
//! project run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::too_many_struct_fields";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_fields: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_every_field_count() {
    dylint_testing::ui::Test::src_base(
        env!("CARGO_PKG_NAME"),
        "ui-toml/too_many_struct_fields/zero_threshold",
    )
    .dylint_toml(dylint_toml(RuleConfig {
        max_fields: Some(0),
    }))
    .run();
}

/// A library whose production struct, `#[cfg(test)]` fixture struct, and
/// struct local to a `#[test]` each have eleven fields — one above the
/// default limit.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/too_many_struct_fields/lib_with_test_module.rs");

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
    let expected = format!("struct `{function}` has");
    assert!(
        stderr.contains(&expected),
        "expected `{function}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, function: &str) {
    let unexpected = format!("struct `{function}` has");
    assert!(
        !stderr.contains(&unexpected),
        "expected `{function}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_tmsf_default", "");
    assert_flagged(&stderr, "Production");
    assert_flagged(&stderr, "CfgTestFixture");
    assert_flagged(&stderr, "InTest");
}

#[test]
fn test_code_exception_leaves_test_code_alone() {
    let stderr = run(
        "fixture_tmsf_test_exception",
        text_block_fnl! {
            r#"["perfectionist::too_many_struct_fields"]"#
            "test_code_exception = true"
        },
    );
    assert_flagged(&stderr, "Production");
    assert_not_flagged(&stderr, "CfgTestFixture");
    assert_not_flagged(&stderr, "InTest");
}
