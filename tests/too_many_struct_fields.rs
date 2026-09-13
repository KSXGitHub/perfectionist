//! Tests for `too_many_struct_fields`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/too_many_struct_fields.rs`
//! and is picked up by `tests/ui.rs`. The `max_fields` knob is
//! covered by a UI fixture under `ui-toml/too_many_struct_fields/` run
//! with a per-rule `dylint.toml`; `exempt_tests` needs
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
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/too_many_struct_fields/zero_threshold",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
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

const LIB_SOURCES: &[(&str, &str)] = &[("src/lib.rs", LIB_WITH_TEST_MODULE)];

/// The same over-limit struct in an integration test and in a
/// benchmark. Neither carries a `#[cfg(test)]` gate nor a `#[test]`
/// function, so they are test code only through the Cargo target they
/// sit in -- the half of `item_in_test_code` that no attribute reaches.
const TARGET_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "tests/it.rs",
        include_str!("fixtures/too_many_struct_fields/target_struct.rs"),
    ),
    (
        "benches/bench.rs",
        include_str!("fixtures/too_many_struct_fields/target_struct.rs"),
    ),
];

/// Run the fixture and return its stderr, asserting that `cargo dylint`
/// itself succeeded.
fn run(package_name: &str, sources: &[(&str, &str)], config: &str) -> String {
    let (_temp, stderr, success) = run_project_with_config(
        package_name,
        cargo_manifest_dir(),
        &shared_target_dir(),
        sources,
        config,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

fn assert_flagged(stderr: &str, name: &str) {
    let expected = format!("struct `{name}` has");
    assert!(
        stderr.contains(&expected),
        "expected `{name}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, name: &str) {
    let unexpected = format!("struct `{name}` has");
    assert!(
        !stderr.contains(&unexpected),
        "expected `{name}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_tmsf_default", LIB_SOURCES, "");
    assert_flagged(&stderr, "Production");
    assert_flagged(&stderr, "CfgTestFixture");
    assert_flagged(&stderr, "InTest");
}

#[test]
fn exempt_tests_leaves_test_code_alone() {
    let stderr = run(
        "fixture_tmsf_test_exception",
        LIB_SOURCES,
        text_block_fnl! {
            r#"["perfectionist::too_many_struct_fields"]"#
            "exempt_tests = true"
        },
    );
    assert_flagged(&stderr, "Production");
    assert_not_flagged(&stderr, "CfgTestFixture");
    assert_not_flagged(&stderr, "InTest");
}

#[test]
fn a_test_target_is_measured_by_default() {
    let stderr = run("fixture_tmsf_target_default", TARGET_SOURCES, "");
    assert_flagged(&stderr, "InTarget");
}

#[test]
fn exempt_tests_leaves_a_test_target_alone() {
    let stderr = run(
        "fixture_tmsf_target_exception",
        TARGET_SOURCES,
        text_block_fnl! {
            r#"["perfectionist::too_many_struct_fields"]"#
            "exempt_tests = true"
        },
    );
    assert_not_flagged(&stderr, "InTarget");
}
