//! Integration tests for `cloning_getter`'s `test_code_exception`,
//! which needs `#[cfg(test)]` code to exist and so runs a minimal Cargo
//! project through `cargo dylint --all -- --all-targets`, the way
//! `tests/needless_borrowed_parameters.rs` does it. The default-config
//! sweep lives in `ui/cloning_getter.rs`.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use text_block_macros::text_block_fnl;

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
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_cg_default", "");
    assert_flagged(&stderr, "name");
    assert_flagged(&stderr, "label");
}

#[test]
fn test_code_exception_leaves_test_code_alone() {
    let stderr = run(
        "fixture_cg_test_exception",
        text_block_fnl! {
            r#"["perfectionist::cloning_getter"]"#
            "test_code_exception = true"
        },
    );
    assert_flagged(&stderr, "name");
    assert_not_flagged(&stderr, "label");
}
