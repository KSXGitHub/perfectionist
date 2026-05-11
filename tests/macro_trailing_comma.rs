//! Integration tests for `macro_trailing_comma`'s configuration
//! knobs. The UI test (`ui/macro_trailing_comma.rs`) covers the rule's
//! built-in name-based set with the default config; these tests
//! exercise `extra_name_based` and `ignore` by injecting a per-rule
//! configuration table into the fixture's `dylint.toml`.
//!
//! Each test materialises a minimal Cargo project under a `TempDir`,
//! writes a `dylint.toml` that points at the perfectionist crate and
//! carries a `["perfectionist::macro_trailing_comma"]` config table,
//! and runs `cargo dylint --all`. Every test shares
//! `<workspace>/target/integration-fixtures` as `CARGO_TARGET_DIR` so
//! cargo's compilation cache is reused; each fixture has a unique
//! package name to avoid cargo treating them as the same project.
//! Pre-warm with `just warmup-integration-tests`.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_sources_and_dylint_config, shared_target_dir};

const LINT_NAME: &str = "perfectionist::macro_trailing_comma";

/// Fixture body that defines an uncurated `macro_rules!` macro and
/// invokes it across multiple lines without a trailing comma. With
/// the default config this invocation is *not* flagged (the macro is
/// not on the built-in list); a test enables `extra_name_based` to
/// confirm the entry takes effect.
const UNCURATED_MULTI_LINE_FIXTURE: &str = "\
macro_rules! my_macro {
    ($($item:expr),* $(,)?) => {{ $(let _ = $item;)* 0 }};
}

pub fn _trigger() {
    let _ = my_macro!(
        1,
        2,
        3
    );
}
";

#[test]
fn extra_name_based_enables_a_user_named_macro() {
    let target = shared_target_dir();
    let extra = format!("[\"{LINT_NAME}\"]\nextra_name_based = [\"my_macro\"]\n");
    let (_temp, stderr, success) = run_project_with_sources_and_dylint_config(
        "fixture_macro_trailing_comma_extra_name_based",
        cargo_manifest_dir(),
        &target,
        &[("src/lib.rs", UNCURATED_MULTI_LINE_FIXTURE)],
        &extra,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT_NAME),
        "expected `{LINT_NAME}` warning; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("multi-line macro invocation should end with a trailing comma"),
        "expected the multi-line trailing-comma diagnostic; stderr was:\n{stderr}",
    );
}

#[test]
fn default_config_does_not_flag_an_uncurated_macro() {
    let target = shared_target_dir();
    let (_temp, stderr, success) = run_project_with_sources_and_dylint_config(
        "fixture_macro_trailing_comma_no_extra_config",
        cargo_manifest_dir(),
        &target,
        &[("src/lib.rs", UNCURATED_MULTI_LINE_FIXTURE)],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT_NAME),
        "did not expect `{LINT_NAME}` warning; stderr was:\n{stderr}",
    );
}

#[test]
fn ignore_suppresses_a_built_in_curated_macro() {
    // `vec!` is on the built-in name-based list, so this multi-line
    // invocation would normally be flagged. The `ignore` entry should
    // suppress the diagnostic without disabling the rule altogether.
    let fixture = "\
pub fn _trigger() {
    let _ = vec![
        1,
        2,
        3
    ];
}
";
    let target = shared_target_dir();
    let extra = format!("[\"{LINT_NAME}\"]\nignore = [\"vec\"]\n");
    let (_temp, stderr, success) = run_project_with_sources_and_dylint_config(
        "fixture_macro_trailing_comma_ignore_vec",
        cargo_manifest_dir(),
        &target,
        &[("src/lib.rs", fixture)],
        &extra,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT_NAME),
        "did not expect `{LINT_NAME}` warning; stderr was:\n{stderr}",
    );
}

#[test]
fn ignore_wins_over_extra_name_based_for_the_same_macro() {
    // The same macro appears in both `extra_name_based` and `ignore`.
    // Per the rule spec, `ignore` is checked first, so the invocation
    // is left untouched.
    let target = shared_target_dir();
    let extra = format!(
        "[\"{LINT_NAME}\"]\n\
         extra_name_based = [\"my_macro\"]\n\
         ignore = [\"my_macro\"]\n",
    );
    let (_temp, stderr, success) = run_project_with_sources_and_dylint_config(
        "fixture_macro_trailing_comma_ignore_overrides_extra",
        cargo_manifest_dir(),
        &target,
        &[("src/lib.rs", UNCURATED_MULTI_LINE_FIXTURE)],
        &extra,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT_NAME),
        "`ignore` should win over `extra_name_based`; stderr was:\n{stderr}",
    );
}
