//! Integration tests for `cloning_getter`'s configuration.
//!
//! `getter_name_patterns` is covered by UI fixtures under
//! `ui-toml/cloning_getter/`, each run with a per-rule `dylint.toml`.
//! `exempt_tests` needs `#[cfg(test)]` code to exist, so it runs a
//! minimal Cargo project through `cargo dylint --all -- --all-targets`,
//! the way `tests/needless_borrowed_parameters.rs` does it. The
//! default-config sweep lives in `ui/cloning_getter.rs`.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::cloning_getter";

/// Serialisation shim for the rule's `dylint.toml` configuration, which
/// the test crate cannot build from the lint's own private `Config`.
#[derive(serde::Serialize)]
struct RuleConfig {
    getter_name_patterns: Vec<String>,
}

fn dylint_toml(patterns: &[&str]) -> String {
    let config = RuleConfig {
        getter_name_patterns: patterns.iter().copied().map(str::to_owned).collect(),
    };
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run_patterns(fixture_dir: &str, patterns: &[&str]) {
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base(fixture_dir)
        .dylint_toml(dylint_toml(patterns))
        .run();
}

#[test]
fn a_later_pattern_overrides_an_earlier_one() {
    run_patterns(
        "ui-toml/cloning_getter/measured_by_pattern",
        &["*", "!clone_*"],
    );
}

#[test]
fn a_field_named_getter_is_out_of_the_list_s_reach() {
    run_patterns(
        "ui-toml/cloning_getter/field_names_are_not_overridable",
        &["!*"],
    );
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
