//! Integration tests for `needless_borrowed_parameters`' exemptions.
//!
//! Each test materialises a minimal Cargo project under a `TempDir` and
//! runs `cargo dylint --all -- --all-targets` against it (sharing the
//! warmed `target/integration-fixtures`). `--all-targets` is mandatory
//! twice over: `#[cfg(test)]` code only exists in a build where
//! `cfg(test)` is active, and the integration-test, example, and
//! build-script targets are separate crates that flag adds to the
//! check. Per-rule configuration is appended to the fixture's
//! `dylint.toml` as a quoted
//! `["perfectionist::needless_borrowed_parameters"]` table; pass `""`
//! for the default configuration.
//!
//! The fixture sources live in `fixtures/needless_borrowed_parameters/`
//! and come in via `include_str!`. Each gives its borrowed parameter a
//! distinct name, so an assertion can name the one function it is about
//! instead of counting warnings.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use text_block_macros::text_block_fnl;

const LINT: &str = "perfectionist::needless_borrowed_parameters";

/// Run a fixture and return its stderr, asserting that `cargo dylint`
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

/// Assert that `parameter` was flagged.
fn assert_flagged(stderr: &str, parameter: &str) {
    assert!(
        stderr.contains(LINT),
        "expected a `{LINT}` warning; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains(&format!("parameter `{parameter}` is borrowed")),
        "expected `{parameter}` to be flagged; stderr was:\n{stderr}",
    );
}

/// Assert that `parameter` was not flagged.
fn assert_not_flagged(stderr: &str, parameter: &str) {
    assert!(
        !stderr.contains(&format!("parameter `{parameter}` is borrowed")),
        "expected `{parameter}` to be exempt; stderr was:\n{stderr}",
    );
}

/// A library whose production function is flagged, whose
/// `#[cfg(test)]` module is not, and whose `#[test]` body's nested
/// helper is not either.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/needless_borrowed_parameters/lib_with_test_module.rs");

#[test]
fn production_code_is_still_flagged() {
    let stderr = run(
        "fixture_nbp_production",
        &[("src/lib.rs", LIB_WITH_TEST_MODULE)],
        "",
    );
    assert_flagged(&stderr, "production_param");
}

#[test]
fn does_not_flag_cfg_test_code() {
    let stderr = run(
        "fixture_nbp_cfg_test",
        &[("src/lib.rs", LIB_WITH_TEST_MODULE)],
        "",
    );
    assert_not_flagged(&stderr, "cfg_test_param");
    assert_not_flagged(&stderr, "nested_param");
}

#[test]
fn flags_test_code_when_the_exception_is_off() {
    let stderr = run(
        "fixture_nbp_test_exception_off",
        &[("src/lib.rs", LIB_WITH_TEST_MODULE)],
        text_block_fnl! {
            r#"["perfectionist::needless_borrowed_parameters"]"#
            "exempt_tests = false"
        },
    );
    assert_flagged(&stderr, "cfg_test_param");
    assert_flagged(&stderr, "nested_param");
}

/// A `#[test]` function at file scope, with no `#[cfg(test)]` on it or
/// around it. Its nested helper is reachable only through the
/// `is_in_test_function` half of `in_test_code`; the `cfg` half finds
/// nothing to match.
const LIB_WITH_UNGATED_TEST: &str =
    include_str!("fixtures/needless_borrowed_parameters/lib_with_ungated_test.rs");

#[test]
fn does_not_flag_a_helper_inside_an_ungated_test_function() {
    let stderr = run(
        "fixture_nbp_ungated_test",
        &[("src/lib.rs", LIB_WITH_UNGATED_TEST)],
        "",
    );
    assert_not_flagged(&stderr, "ungated_test_param");
}

#[test]
fn flags_an_ungated_test_helper_when_the_exception_is_off() {
    let stderr = run(
        "fixture_nbp_ungated_test_exception_off",
        &[("src/lib.rs", LIB_WITH_UNGATED_TEST)],
        text_block_fnl! {
            r#"["perfectionist::needless_borrowed_parameters"]"#
            "exempt_tests = false"
        },
    );
    assert_flagged(&stderr, "ungated_test_param");
}

/// The compound-predicate cases: `all(...)` is test-only as soon as one
/// conjunct is, `not(...)` composes by De Morgan so a double negation
/// is still test-only, and `any(...)` is test-only only if *every*
/// branch is — `any(test, <anything else>)` can hold in a build without
/// `test`, so it is production code as far as the rule is concerned.
///
/// The `all(...)` conjunct is `debug_assertions` rather than something
/// like `unix` so that the item exists on every platform. Under a
/// conjunct that is false for the host, it would be configured out and
/// the assertion would pass without the rule having looked at it.
const LIB_WITH_COMPOUND_CFGS: &str =
    include_str!("fixtures/needless_borrowed_parameters/lib_with_compound_cfgs.rs");

#[test]
fn does_not_flag_compound_cfg_test_predicates() {
    let stderr = run(
        "fixture_nbp_compound_cfg",
        &[("src/lib.rs", LIB_WITH_COMPOUND_CFGS)],
        "",
    );
    assert_not_flagged(&stderr, "conjunction_param");
    assert_not_flagged(&stderr, "double_negation_param");
    assert_not_flagged(&stderr, "negated_disjunction_param");
}

/// Guards the fixture above against going vacuous. With the exemption
/// off, every one of its functions must be flagged; if a `cfg`
/// conjunct ever configures one out of the test build, this fails
/// instead of the exemption test quietly passing on nothing.
#[test]
fn every_compound_cfg_function_reaches_the_rule() {
    let stderr = run(
        "fixture_nbp_compound_cfg_exception_off",
        &[("src/lib.rs", LIB_WITH_COMPOUND_CFGS)],
        text_block_fnl! {
            r#"["perfectionist::needless_borrowed_parameters"]"#
            "exempt_tests = false"
        },
    );
    assert_flagged(&stderr, "conjunction_param");
    assert_flagged(&stderr, "double_negation_param");
    assert_flagged(&stderr, "disjunction_param");
    assert_flagged(&stderr, "negated_conjunction_param");
    assert_flagged(&stderr, "negated_disjunction_param");
}

#[test]
fn flags_a_cfg_predicate_that_admits_more_than_test() {
    let stderr = run(
        "fixture_nbp_disjunction_cfg",
        &[("src/lib.rs", LIB_WITH_COMPOUND_CFGS)],
        "",
    );
    assert_flagged(&stderr, "disjunction_param");
    assert_flagged(&stderr, "negated_conjunction_param");
}

/// The separate-target fixture: an integration test and a benchmark
/// are wholly test code, while an example is documentation held to the
/// library's standard.
const SEPARATE_TARGET_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "tests/it.rs",
        include_str!("fixtures/needless_borrowed_parameters/integration_test.rs"),
    ),
    (
        "benches/bench.rs",
        include_str!("fixtures/needless_borrowed_parameters/benchmark.rs"),
    ),
    (
        "examples/demo.rs",
        include_str!("fixtures/needless_borrowed_parameters/example.rs"),
    ),
];

#[test]
fn does_not_flag_an_integration_test_or_benchmark_crate() {
    let stderr = run(
        "fixture_nbp_integration_target",
        SEPARATE_TARGET_SOURCES,
        "",
    );
    assert_not_flagged(&stderr, "integration_param");
    assert_not_flagged(&stderr, "benchmark_param");
}

#[test]
fn flags_an_example_crate() {
    let stderr = run("fixture_nbp_example_target", SEPARATE_TARGET_SOURCES, "");
    assert_flagged(&stderr, "example_param");
}

#[test]
fn flags_a_test_target_when_the_exception_is_off() {
    let stderr = run(
        "fixture_nbp_integration_exception_off",
        SEPARATE_TARGET_SOURCES,
        text_block_fnl! {
            r#"["perfectionist::needless_borrowed_parameters"]"#
            "exempt_tests = false"
        },
    );
    assert_flagged(&stderr, "integration_param");
    assert_flagged(&stderr, "benchmark_param");
}

/// A package whose build script holds the violation. Cargo picks
/// `build.rs` up without a `build` key, and compiles it as its own
/// crate under a `build_script_*` crate name.
const BUILD_SCRIPT_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "build.rs",
        include_str!("fixtures/needless_borrowed_parameters/build_script.rs"),
    ),
];

#[test]
fn does_not_flag_a_build_script() {
    let stderr = run("fixture_nbp_build_script", BUILD_SCRIPT_SOURCES, "");
    assert_not_flagged(&stderr, "build_script_param");
}

#[test]
fn flags_a_build_script_when_the_exception_is_off() {
    let stderr = run(
        "fixture_nbp_build_script_exception_off",
        BUILD_SCRIPT_SOURCES,
        text_block_fnl! {
            r#"["perfectionist::needless_borrowed_parameters"]"#
            "exempt_build_scripts = false"
        },
    );
    assert_flagged(&stderr, "build_script_param");
}
