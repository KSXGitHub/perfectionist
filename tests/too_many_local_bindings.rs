//! Tests for `too_many_local_bindings`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/too_many_local_bindings.rs`
//! and is picked up by `tests/ui.rs`. The `max_bindings` knob is
//! covered by a UI fixture under `ui-toml/too_many_local_bindings/` run
//! with a per-rule `dylint.toml`; `exempt_tests` needs
//! `#[cfg(test)]` code to exist, so it is covered by a minimal Cargo
//! project run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::too_many_local_bindings";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_bindings: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_every_binding_shape() {
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/too_many_local_bindings/zero_threshold",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig {
            max_bindings: Some(0),
        }))
        .run();
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each bind sixteen names — one above the default
/// limit.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/too_many_local_bindings/lib_with_test_module.rs");

/// A library whose only source is that fixture.
const LIB_SOURCES: &[(&str, &str)] = &[("src/lib.rs", LIB_WITH_TEST_MODULE)];

/// An integration test and a benchmark are wholly test code by virtue of
/// the Cargo target they sit in, with no `#[cfg(test)]` gate or `#[test]`
/// attribute on the flagged function itself. `exempt_tests` has to reach
/// them through the target as well.
const SEPARATE_TARGET_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "tests/it.rs",
        include_str!("fixtures/too_many_local_bindings/integration_test.rs"),
    ),
    (
        "benches/bench.rs",
        include_str!("fixtures/too_many_local_bindings/benchmark.rs"),
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

fn assert_flagged(stderr: &str, function: &str) {
    let expected = format!("function `{function}` binds");
    assert!(
        stderr.contains(&expected),
        "expected `{function}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, function: &str) {
    let unexpected = format!("function `{function}` binds");
    assert!(
        !stderr.contains(&unexpected),
        "expected `{function}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_counted_by_default() {
    let stderr = run("fixture_tmlb_default", LIB_SOURCES, "");
    assert_flagged(&stderr, "production");
    assert_flagged(&stderr, "cfg_test_helper");
    assert_flagged(&stderr, "test_function");
}

#[test]
fn exempt_tests_leaves_test_code_alone() {
    let stderr = run(
        "fixture_tmlb_test_exception",
        LIB_SOURCES,
        text_block_fnl! {
            r#"["perfectionist::too_many_local_bindings"]"#
            "exempt_tests = true"
        },
    );
    assert_flagged(&stderr, "production");
    assert_not_flagged(&stderr, "cfg_test_helper");
    assert_not_flagged(&stderr, "test_function");
}

#[test]
fn counts_a_test_target_by_default() {
    let stderr = run("fixture_tmlb_target_default", SEPARATE_TARGET_SOURCES, "");
    assert_flagged(&stderr, "integration_binder");
    assert_flagged(&stderr, "benchmark_binder");
}

#[test]
fn exempt_tests_leaves_a_test_target_alone() {
    let stderr = run(
        "fixture_tmlb_target_exempt",
        SEPARATE_TARGET_SOURCES,
        text_block_fnl! {
            r#"["perfectionist::too_many_local_bindings"]"#
            "exempt_tests = true"
        },
    );
    assert_not_flagged(&stderr, "integration_binder");
    assert_not_flagged(&stderr, "benchmark_binder");
}
