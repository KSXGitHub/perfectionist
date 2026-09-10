//! Tests for `overly_long_file`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/overly_long_file.rs` and is
//! picked up by `tests/ui.rs`. The `max_lines` knob is covered by a UI
//! fixture under `ui-toml/overly_long_file/` run with a per-rule
//! `dylint.toml`; out-of-line modules and `exempt_tests` need a
//! real crate layout, so they are covered by a minimal Cargo project run
//! through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::overly_long_file";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_tests: Option<bool>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_the_file_count() {
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/overly_long_file/zero_threshold",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig {
            max_lines: Some(0),
            ..RuleConfig::default()
        }))
        .run();
}

#[test]
fn a_file_at_the_limit_is_not_flagged() {
    let fixtures = _utils::copy_fixtures_with_directives(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/overly_long_file/at_the_limit",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig {
            max_lines: Some(5),
            ..RuleConfig::default()
        }))
        .run();
}

/// A crate whose root is 4 lines, and whose `big.rs` module,
/// `#[cfg(test)] mod tests;` file, integration test, and benchmark each
/// hold over 500 lines of code.
///
/// The last two carry no `#[cfg(test)]` gate: they are test code by
/// virtue of the Cargo target they sit in, which `exempt_tests` has to
/// reach through the target rather than through an attribute.
const SOURCES: &[(&str, &str)] = &[
    (
        "src/lib.rs",
        include_str!("fixtures/overly_long_file/lib.rs"),
    ),
    (
        "src/big.rs",
        include_str!("fixtures/overly_long_file/big.rs"),
    ),
    (
        "src/tests.rs",
        include_str!("fixtures/overly_long_file/tests.rs"),
    ),
    (
        "tests/it.rs",
        include_str!("fixtures/overly_long_file/big.rs"),
    ),
    (
        "benches/bench.rs",
        include_str!("fixtures/overly_long_file/big.rs"),
    ),
];

/// Run the fixture and return its stderr, asserting that `cargo dylint`
/// itself succeeded.
fn run(package_name: &str, config: &str) -> String {
    let (_temp, stderr, success) = run_project_with_config(
        package_name,
        cargo_manifest_dir(),
        &shared_target_dir(),
        SOURCES,
        config,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

fn assert_flagged(stderr: &str, file: &str) {
    let expected = format!("{file}` has");
    assert!(
        stderr.contains(&expected),
        "expected `{file}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, file: &str) {
    let unexpected = format!("{file}` has");
    assert!(
        !stderr.contains(&unexpected),
        "expected `{file}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn out_of_line_modules_are_measured_and_test_files_count_by_default() {
    let stderr = run("fixture_olfile_default", "");
    assert_flagged(&stderr, "big.rs");
    assert_flagged(&stderr, "tests.rs");
    assert_flagged(&stderr, "it.rs");
    assert_flagged(&stderr, "bench.rs");
    assert_not_flagged(&stderr, "lib.rs");
}

#[test]
fn exempt_tests_leaves_test_files_alone() {
    let stderr = run(
        "fixture_olfile_test_exception",
        text_block_fnl! {
            r#"["perfectionist::overly_long_file"]"#
            "exempt_tests = true"
        },
    );
    assert_flagged(&stderr, "big.rs");
    assert_not_flagged(&stderr, "tests.rs");
    assert_not_flagged(&stderr, "it.rs");
    assert_not_flagged(&stderr, "bench.rs");
}
