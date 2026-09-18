//! Tests for `overly_complex_condition`'s configuration knobs.
//!
//! The default-config sweep lives in `ui/overly_complex_condition.rs`
//! and is picked up by `tests/ui.rs`. The `max_operators` knob is
//! covered by a UI fixture under `ui-toml/overly_complex_condition/`
//! run with a per-rule `dylint.toml`. Holding test code to the same
//! limit as the code it exercises needs `#[cfg(test)]` code and real
//! Cargo targets to exist, so it is covered by a minimal Cargo project
//! run through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use std::collections::BTreeMap;

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
    _utils::configured_ui_test(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_MANIFEST_DIR"),
        "ui-toml/overly_complex_condition/zero_threshold",
        dylint_toml(RuleConfig {
            max_operators: Some(0),
        }),
    )
    .run();
}

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each hold a condition of four operators — one
/// above the default limit. The conditions are the only ones in the
/// fixture, so a flag is identified by the line it points at.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/overly_complex_condition/lib_with_test_module.rs");

const LIB_SOURCES: &[(&str, &str)] = &[("src/lib.rs", LIB_WITH_TEST_MODULE)];

/// The same over-limit condition in an integration test and in a
/// benchmark. Neither carries a `#[cfg(test)]` gate nor a `#[test]`
/// function, so nothing but the Cargo target they sit in marks them as
/// test code.
const TARGET_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "tests/it.rs",
        include_str!("fixtures/overly_complex_condition/target_condition.rs"),
    ),
    (
        "benches/bench.rs",
        include_str!("fixtures/overly_complex_condition/target_condition.rs"),
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

fn assert_flagged(stderr: &str, location: &str) {
    assert!(
        stderr.contains(location),
        "expected `{location}` to be flagged; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_occ_default", LIB_SOURCES, "");
    assert_flagged(&stderr, "src/lib.rs:2:8");
    assert_flagged(&stderr, "src/lib.rs:8:12");
    assert_flagged(&stderr, "src/lib.rs:14:12");
}

#[test]
fn a_test_target_is_measured_by_default() {
    let stderr = run("fixture_occ_target_default", TARGET_SOURCES, "");
    assert_flagged(&stderr, "tests/it.rs:6:8");
    assert_flagged(&stderr, "benches/bench.rs:6:8");
}
