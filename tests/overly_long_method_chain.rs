//! Tests for `overly_long_method_chain`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/overly_long_method_chain.rs`
//! and is picked up by `tests/ui.rs`. The `max_calls` knob is
//! covered by a UI fixture under `ui-toml/overly_long_method_chain/` run
//! with a per-rule `dylint.toml`; `test_code_exception` needs
//! `#[cfg(test)]` code to exist, so it is covered by a minimal Cargo
//! project run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::overly_long_method_chain";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_calls: Option<usize>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

#[test]
fn zero_threshold_reports_every_chain_length() {
    let fixtures = _utils::copy_fixtures_with_directive(
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/overly_long_method_chain/zero_threshold",
    );
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(RuleConfig { max_calls: Some(0) }))
        .run();
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each bind sixteen names — one above the default
/// limit.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/overly_long_method_chain/lib_with_test_module.rs");

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

/// The chains are the only ones in the fixture, so a flag is identified
/// by the line the diagnostic points at rather than a function name.
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
    let stderr = run("fixture_olmc_default", "");
    assert_flagged(&stderr, "src/lib.rs:2:5");
    assert_flagged(&stderr, "src/lib.rs:14:9");
    assert_flagged(&stderr, "src/lib.rs:26:21");
}

#[test]
fn test_code_exception_leaves_test_code_alone() {
    let stderr = run(
        "fixture_olmc_test_exception",
        text_block_fnl! {
            r#"["perfectionist::overly_long_method_chain"]"#
            "test_code_exception = true"
        },
    );
    assert_flagged(&stderr, "src/lib.rs:2:5");
    assert_not_flagged(&stderr, "src/lib.rs:14:9");
    assert_not_flagged(&stderr, "src/lib.rs:26:21");
}
