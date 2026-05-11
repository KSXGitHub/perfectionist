//! Integration test for `flat_module_pattern`.
//!
//! Each test materialises a minimal Cargo project under a `TempDir`,
//! points its `dylint.toml` at the perfectionist crate this test ships
//! with, and runs `cargo dylint --all` against it. Every test shares
//! `<workspace>/target/integration-fixtures` as `CARGO_TARGET_DIR` so
//! cargo's compilation cache (std, dylint plugin) is reused; each
//! fixture has a unique package name to avoid cargo treating them as
//! the same project. Pre-warm with `just warmup-integration-tests`.

pub mod _utils;

use _utils::{perfectionist_dir, run_project_with_sources, shared_target_dir};

#[test]
fn flags_mod_rs_submodule() {
    let target = shared_target_dir();
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_flags_mod_rs_submodule",
        perfectionist_dir(),
        &target,
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo/mod.rs", "pub fn bar() {}\n"),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("perfectionist::flat_module_pattern"),
        "expected `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("submodule uses the `mod.rs` layout"),
        "expected lint message; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("foo/mod.rs"),
        "expected reference to `foo/mod.rs`; stderr was:\n{stderr}"
    );
}

#[test]
fn does_not_flag_flat_layout() {
    let target = shared_target_dir();
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_does_not_flag_flat_layout",
        perfectionist_dir(),
        &target,
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo.rs", "pub fn bar() {}\n"),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains("perfectionist::flat_module_pattern"),
        "did not expect `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
}

#[test]
fn does_not_flag_deep_flat_layout() {
    let target = shared_target_dir();
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_does_not_flag_deep_flat_layout",
        perfectionist_dir(),
        &target,
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo.rs", "pub mod bar;\n"),
            ("src/foo/bar.rs", "pub mod baz;\n"),
            ("src/foo/bar/baz.rs", ""),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains("perfectionist::flat_module_pattern"),
        "did not expect `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
}

#[test]
fn flags_deep_mod_rs_at_leaf_level() {
    let target = shared_target_dir();
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_flags_deep_mod_rs_at_leaf_level",
        perfectionist_dir(),
        &target,
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo.rs", "pub mod bar;\n"),
            ("src/foo/bar/mod.rs", "pub mod baz;\n"),
            ("src/foo/bar/baz.rs", ""),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("perfectionist::flat_module_pattern"),
        "expected `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("foo/bar/mod.rs"),
        "expected reference to `foo/bar/mod.rs`; stderr was:\n{stderr}"
    );
}

#[test]
fn flags_mod_rs_at_intermediate_level() {
    let target = shared_target_dir();
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_flags_mod_rs_at_intermediate_level",
        perfectionist_dir(),
        &target,
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo/mod.rs", "pub mod bar;\n"),
            ("src/foo/bar.rs", "pub mod baz;\n"),
            ("src/foo/bar/baz.rs", ""),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("perfectionist::flat_module_pattern"),
        "expected `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("foo/mod.rs"),
        "expected reference to `foo/mod.rs`; stderr was:\n{stderr}"
    );
}
