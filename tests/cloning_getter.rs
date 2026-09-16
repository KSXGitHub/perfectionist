//! Integration tests for `cloning_getter`'s configuration.
//!
//! `measure_unmatched_names` is covered by a UI fixture under
//! `ui-toml/cloning_getter/` run with a per-rule `dylint.toml`.
//! `exempt_tests`
//! which needs `#[cfg(test)]` code to exist and so runs a minimal Cargo
//! project through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it. The default-config
//! sweep lives in `ui/cloning_getter.rs`.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::cloning_getter";

/// Serialisation shim for the rule's `dylint.toml` configuration, which
/// the test crate cannot build from the lint's own private `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    measure_unmatched_names: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_exempt_prefixes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_exempt_prefixes: Option<Vec<String>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn unmatched_names_admits_an_unrelated_name() {
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/cloning_getter/unmatched_names",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig {
            measure_unmatched_names: Some(true),
            ..RuleConfig::default()
        }))
        .run();
}

#[test]
fn the_exempt_prefix_roster_is_configurable() {
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/cloning_getter/exempt_prefixes",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig {
            measure_unmatched_names: Some(true),
            extra_exempt_prefixes: Some(vec!["copy_".to_owned()]),
            ignore_exempt_prefixes: Some(vec!["cloned_".to_owned()]),
        }))
        .run();
}

/// A library with a cloning getter in production code and another in a
/// `#[cfg(test)]` module.
const LIB_WITH_TEST_MODULE: &str = include_str!("fixtures/cloning_getter/lib_with_test_module.rs");

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

fn assert_flagged(stderr: &str, getter: &str) {
    let expected = format!("getter `{getter}` returns");
    assert!(
        stderr.contains(&expected),
        "expected `{getter}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, getter: &str) {
    let unexpected = format!("getter `{getter}` returns");
    assert!(
        !stderr.contains(&unexpected),
        "expected `{getter}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_left_alone_by_default() {
    let stderr = run("fixture_cg_default", "");
    assert_flagged(&stderr, "name");
    assert_not_flagged(&stderr, "label");
}

#[test]
fn exempt_tests_off_measures_test_code() {
    let stderr = run(
        "fixture_cg_test_measured",
        text_block_fnl! {
            r#"["perfectionist::cloning_getter"]"#
            "exempt_tests = false"
        },
    );
    assert_flagged(&stderr, "name");
    assert_flagged(&stderr, "label");
}
