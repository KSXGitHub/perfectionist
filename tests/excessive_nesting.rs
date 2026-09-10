//! Tests for `excessive_nesting`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/excessive_nesting.rs`
//! and is picked up by `tests/ui.rs`. The `max_depth` knob is
//! covered by a UI fixture under `ui-toml/excessive_nesting/` run
//! with a per-rule `dylint.toml`; `exempt_tests` needs
//! `#[cfg(test)]` code to exist, so it is covered by a minimal Cargo
//! project run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::excessive_nesting";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_depth: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_every_depth() {
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/excessive_nesting/zero_threshold",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig { max_depth: Some(0) }))
        .run();
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each nest 4 levels deep — 1 above the
/// default limit.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/excessive_nesting/lib_with_test_module.rs");

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

/// Whether `stderr` carries *this* rule's diagnostic for `function`.
///
/// Keyed on the name and the verb rather than on the whole opening
/// phrase: the diagnostic calls a method a `method`, so a fixture that
/// grew one would not match a hardcoded `function` prefix. `nests` is
/// this rule's own verb, so the pair is specific enough.
fn is_flagged(stderr: &str, function: &str) -> bool {
    let phrase = format!("`{function}` nests");
    stderr.lines().any(|line| line.contains(&phrase))
}

fn assert_flagged(stderr: &str, function: &str) {
    assert!(
        is_flagged(stderr, function),
        "expected `{function}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, function: &str) {
    assert!(
        !is_flagged(stderr, function),
        "expected `{function}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_en_default", "");
    assert_flagged(&stderr, "production");
    assert_flagged(&stderr, "cfg_test_helper");
    assert_flagged(&stderr, "test_function");
}

#[test]
fn exempt_tests_leaves_test_code_alone() {
    let stderr = run(
        "fixture_en_test_exception",
        text_block_fnl! {
            r#"["perfectionist::excessive_nesting"]"#
            "exempt_tests = true"
        },
    );
    assert_flagged(&stderr, "production");
    assert_not_flagged(&stderr, "cfg_test_helper");
    assert_not_flagged(&stderr, "test_function");
}
