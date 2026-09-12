//! Tests for `overly_long_method_chain`.
//!
//! The rule is inactive by default, so there is no default-config
//! sweep under `ui/` for `tests/ui.rs` to pick up: `ui/overly_long_
//! method_chain.rs` holds two over-limit chains and an empty `.stderr`,
//! which is what proves the rule stays off until it is enabled. Every
//! fixture here therefore travels with a `dylint.toml` that enables it
//! — `default` pins the default limit, `zero_threshold` pins where a
//! chain starts and ends. `exempt_tests` needs `#[cfg(test)]` code to
//! exist, so it is covered by a minimal Cargo project run through
//! `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::overly_long_method_chain";
const RULE_NAME: &str = "overly_long_method_chain";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_calls: Option<usize>,
}

/// The `[perfectionist]` table, kept minimal so every fixture below
/// turns the rule on. The rule ships inactive by default
/// (`DEFAULT_STATE = DefaultState::Inactive` in
/// `src/rules/overly_long_method_chain.rs`), so without this `enable`
/// entry the pass would never register and the fixtures' over-limit
/// chains wouldn't trigger a diagnostic.
#[derive(serde::Serialize)]
struct GlobalConfig {
    enable: Vec<&'static str>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // Serialise as two top-level tables in one call, the way
    // `tests/unordered_derives.rs` does: building the string by
    // concatenation would risk the `[perfectionist]` table's contents
    // bleeding into the rule table when the rule config is empty.
    #[derive(serde::Serialize)]
    struct WholeToml<'a> {
        perfectionist: GlobalConfig,
        #[serde(flatten)]
        rule: BTreeMap<&'a str, RuleConfig>,
    }
    let whole = WholeToml {
        perfectionist: GlobalConfig {
            enable: vec![RULE_NAME],
        },
        rule: [(LINT_NAME, config)].into_iter().collect(),
    };
    toml::to_string(&whole).expect("serialise rule config as dylint.toml")
}

fn run_ui(src_base: &str, config: RuleConfig) {
    let fixtures = _utils::copy_fixtures_with_directives(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn default_limit_flags_a_chain_above_five_calls() {
    // No `max_calls` override: the fixture is measured against the
    // rule's own default of 5.
    run_ui(
        "ui-toml/overly_long_method_chain/default",
        RuleConfig::default(),
    );
}

#[test]
fn zero_threshold_reports_every_chain_length() {
    run_ui(
        "ui-toml/overly_long_method_chain/zero_threshold",
        RuleConfig { max_calls: Some(0) },
    );
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each hold the same 6-call chain — one above the
/// default limit of 5.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/overly_long_method_chain/lib_with_test_module.rs");

/// Run the fixture and return its stderr, asserting that `cargo dylint`
/// itself succeeded. `extra` is appended to the `[perfectionist]` table
/// that enables the rule.
fn run(package_name: &str, extra: &str) -> String {
    let config = format!(
        "{}\n{extra}",
        text_block_fnl! {
            "[perfectionist]"
            r#"enable = ["overly_long_method_chain"]"#
        },
    );
    let (_temp, stderr, success) = run_project_with_config(
        package_name,
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", LIB_WITH_TEST_MODULE)],
        &config,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

/// The three chains are identical, so a flag is identified by the
/// `line:column` the diagnostic points at rather than by a name.
fn assert_flagged(stderr: &str, location: &str) {
    assert!(
        stderr.contains(location),
        "expected the chain at `{location}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, location: &str) {
    assert!(
        !stderr.contains(location),
        "expected the chain at `{location}` to be exempt; stderr was:\n{stderr}",
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
fn exempt_tests_leaves_test_code_alone() {
    let stderr = run(
        "fixture_olmc_test_exception",
        text_block_fnl! {
            r#"["perfectionist::overly_long_method_chain"]"#
            "exempt_tests = true"
        },
    );
    assert_flagged(&stderr, "src/lib.rs:2:5");
    assert_not_flagged(&stderr, "src/lib.rs:14:9");
    assert_not_flagged(&stderr, "src/lib.rs:26:21");
}
