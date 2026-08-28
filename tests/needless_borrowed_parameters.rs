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
//! Every fixture gives its borrowed parameter a distinct name, so an
//! assertion can name the one function it is about instead of counting
//! warnings.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};

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
const LIB_WITH_TEST_MODULE: &str = "\
pub fn production(production_param: &str) -> String {
    production_param.to_owned()
}

#[cfg(test)]
mod tests {
    pub fn helper(cfg_test_param: &str) -> String {
        cfg_test_param.to_owned()
    }

    #[test]
    fn nested() {
        fn nested_helper(nested_param: &str) -> String {
            nested_param.to_owned()
        }
        assert_eq!(helper(\"a\"), nested_helper(\"a\"));
    }
}
";

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
        "[\"perfectionist::needless_borrowed_parameters\"]\ntest_code_exception = false\n",
    );
    assert_flagged(&stderr, "cfg_test_param");
    assert_flagged(&stderr, "nested_param");
}

/// The compound-predicate cases: `all(...)` is test-only as soon as one
/// conjunct is, `not(...)` composes by De Morgan so a double negation
/// is still test-only, and `any(...)` is test-only only if *every*
/// branch is — `any(test, <anything else>)` can hold in a build without
/// `test`, so it is production code as far as the rule is concerned.
const LIB_WITH_COMPOUND_CFGS: &str = "\
#[cfg(all(test, unix))]
fn conjunction(conjunction_param: &str) -> String {
    conjunction_param.to_owned()
}

#[cfg(not(not(test)))]
fn double_negation(double_negation_param: &str) -> String {
    double_negation_param.to_owned()
}

#[cfg(any(test, target_pointer_width = \"64\"))]
fn disjunction(disjunction_param: &str) -> String {
    disjunction_param.to_owned()
}
";

#[test]
fn does_not_flag_compound_cfg_test_predicates() {
    let stderr = run(
        "fixture_nbp_compound_cfg",
        &[("src/lib.rs", LIB_WITH_COMPOUND_CFGS)],
        "",
    );
    assert_not_flagged(&stderr, "conjunction_param");
    assert_not_flagged(&stderr, "double_negation_param");
}

#[test]
fn flags_a_cfg_predicate_that_only_admits_test() {
    let stderr = run(
        "fixture_nbp_disjunction_cfg",
        &[("src/lib.rs", LIB_WITH_COMPOUND_CFGS)],
        "",
    );
    assert_flagged(&stderr, "disjunction_param");
}

/// The separate-target fixture: an integration test and a benchmark
/// are wholly test code, while an example is documentation held to the
/// library's standard.
const SEPARATE_TARGET_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "tests/it.rs",
        "\
fn helper(integration_param: &str) -> String {
    integration_param.to_owned()
}

#[test]
fn works() {
    assert_eq!(helper(\"a\"), \"a\");
}
",
    ),
    (
        "examples/demo.rs",
        "\
fn helper(example_param: &str) -> String {
    example_param.to_owned()
}

fn main() {
    println!(\"{}\", helper(\"a\"));
}
",
    ),
];

#[test]
fn does_not_flag_an_integration_test_crate() {
    let stderr = run(
        "fixture_nbp_integration_target",
        SEPARATE_TARGET_SOURCES,
        "",
    );
    assert_not_flagged(&stderr, "integration_param");
}

#[test]
fn flags_an_example_crate() {
    let stderr = run("fixture_nbp_example_target", SEPARATE_TARGET_SOURCES, "");
    assert_flagged(&stderr, "example_param");
}

#[test]
fn flags_an_integration_test_crate_when_the_exception_is_off() {
    let stderr = run(
        "fixture_nbp_integration_exception_off",
        SEPARATE_TARGET_SOURCES,
        "[\"perfectionist::needless_borrowed_parameters\"]\ntest_code_exception = false\n",
    );
    assert_flagged(&stderr, "integration_param");
}

/// A package whose build script holds the violation. Cargo picks
/// `build.rs` up without a `build` key, and compiles it as its own
/// crate under a `build_script_*` crate name.
const BUILD_SCRIPT_SOURCES: &[(&str, &str)] = &[
    ("src/lib.rs", "pub fn nothing() {}\n"),
    (
        "build.rs",
        "\
fn helper(build_script_param: &str) -> String {
    build_script_param.to_owned()
}

fn main() {
    println!(\"cargo::rerun-if-changed=build.rs\");
    println!(\"cargo::rustc-env=GREETING={}\", helper(\"a\"));
}
",
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
        "[\"perfectionist::needless_borrowed_parameters\"]\nbuild_script_exception = false\n",
    );
    assert_flagged(&stderr, "build_script_param");
}
