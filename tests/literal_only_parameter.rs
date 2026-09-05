//! Integration tests for `literal_only_parameter`. Whether a function
//! is reachable from the crate's public API, and whether it is test
//! code, both need a real crate layout, so these run a minimal Cargo
//! project through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it. The default-config
//! sweep lives in `ui/literal_only_parameter.rs`.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use text_block_macros::text_block_fnl;

/// A library with an exported function, a private one, and a
/// `#[cfg(test)]` helper, each with a `verbose: bool` that every call
/// site passes as a literal.
const LIB: &str = include_str!("fixtures/literal_only_parameter/lib.rs");

fn run(package_name: &str, config: &str) -> String {
    let (_temp, stderr, success) = run_project_with_config(
        package_name,
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", LIB)],
        config,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

fn assert_flagged(stderr: &str, function: &str) {
    let expected = format!("of `{function}` is");
    assert!(
        stderr.contains(&expected),
        "expected `{function}` to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, function: &str) {
    let unexpected = format!("of `{function}` is");
    assert!(
        !stderr.contains(&unexpected),
        "expected `{function}` to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn private_functions_are_judged_and_exported_ones_are_not() {
    let stderr = run("fixture_lop_default", "");
    assert_flagged(&stderr, "private");
    assert_flagged(&stderr, "cfg_test_helper");
    assert_not_flagged(&stderr, "exported");
}

#[test]
fn test_code_exception_leaves_test_code_alone() {
    let stderr = run(
        "fixture_lop_test_exception",
        text_block_fnl! {
            r#"["perfectionist::literal_only_parameter"]"#
            "test_code_exception = true"
        },
    );
    assert_flagged(&stderr, "private");
    assert_not_flagged(&stderr, "cfg_test_helper");
}
