//! Tests for `collection_round_trip`'s configuration knob.
//!
//! The shapes the rule flags, and the ones it leaves alone, live in
//! `ui/collection_round_trip.rs` and are picked up by `tests/ui.rs`.
//! `exempt_tests` needs `#[cfg(test)]` code to exist, so it is covered
//! by a minimal Cargo project run through
//! `cargo dylint --all -- --all-targets`, the way
//! `tests/overly_long_function.rs` does it.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use text_block_macros::text_block_fnl;

/// A library whose production function, `#[cfg(test)]` helper, and
/// `#[test]` function each build a set, walk it again, and build a
/// vector.
const LIB_WITH_TEST_MODULE: &str =
    include_str!("fixtures/collection_round_trip/lib_with_test_module.rs");

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
fn assert_flagged(stderr: &str, line: &str) {
    assert!(
        stderr.contains(line),
        "expected the chain at {line} to be flagged; stderr was:\n{stderr}",
    );
}

fn assert_not_flagged(stderr: &str, line: &str) {
    assert!(
        !stderr.contains(line),
        "expected the chain at {line} to be exempt; stderr was:\n{stderr}",
    );
}

#[test]
fn test_code_is_measured_by_default() {
    let stderr = run("fixture_crt_default", "");
    assert_flagged(&stderr, "src/lib.rs:4:5");
    assert_flagged(&stderr, "src/lib.rs:12:9");
    assert_flagged(&stderr, "src/lib.rs:19:13");
}

#[test]
fn exempt_tests_leaves_test_code_alone() {
    let stderr = run(
        "fixture_crt_test_exception",
        text_block_fnl! {
            r#"["perfectionist::collection_round_trip"]"#
            "exempt_tests = true"
        },
    );
    assert_flagged(&stderr, "src/lib.rs:4:5");
    assert_not_flagged(&stderr, "src/lib.rs:12:9");
    assert_not_flagged(&stderr, "src/lib.rs:19:13");
}
