//! End-to-end proof that `some_bool_comparison`'s rewrite compiles
//! where the rule hands it over as `MachineApplicable`.
//!
//! ## What this actually tests
//!
//! Not the suggestion text — the `ui/` sweep pins that, and a `.stderr`
//! cannot show applicability at all, which is the property that decides
//! whether `cargo dylint --fix` touches a suggestion. Nor the rewrite's
//! *answer*: the unit test beside `src/rules/some_bool_comparison.rs`
//! holds that to the comparison's own in every state. What is left is
//! whether the text the rule splices parses and type-checks where it
//! lands, and that is what the fixer answers.
//!
//! Three shapes earned the test, each a way the rewrite could compile
//! as something other than the comparison it replaced:
//!
//! - A `&bool` payload, where `unwrap_or` alone would hand back a
//!   `&bool` rather than a `bool`.
//! - An option side already behind a reference, where the `unwrap_or`
//!   has to reach the `Option` through it.
//! - A dereferenced option side, where a method call binds tighter than
//!   the `*` and would deref the result instead.
//!
//! A fourth is about where the rewrite lands rather than what it is: a
//! comparison under an `&&` is replaced whole, so the replacement has
//! to bind at least as tightly as the comparison did.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, run_dylint_fix, shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// `needless_borrowed_parameters` would rewrite the signatures the
/// borrowed shapes below depend on, which would make "did the fixer
/// touch this line?" answer the wrong question. The import rules would
/// rewrite the `use` statement on their own account for the same reason.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["import_granularity_mismatch", "import_grouping_mismatch", "needless_borrowed_parameters"]"#
};

const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "use std::collections::HashMap;"
    ""
    "pub fn present_and_true(verbose: Option<bool>) -> bool {"
    "    verbose == Some(true)"
    "}"
    ""
    "pub fn present_and_false(verbose: Option<bool>) -> bool {"
    "    verbose == Some(false)"
    "}"
    ""
    r#"pub fn borrowed_payload(flags: &HashMap<String, bool>) -> bool {"#
    r#"    flags.get("verbose") == Some(&true)"#
    "}"
    ""
    "pub fn borrowed_option(enabled: &Option<bool>) -> bool {"
    "    enabled == &Some(true)"
    "}"
    ""
    "pub fn dereferenced_option(enabled: &Option<bool>) -> bool {"
    "    *enabled == Some(true)"
    "}"
    ""
    "pub fn under_an_and(verbose: Option<bool>, other: bool) -> bool {"
    "    other && verbose != Some(true)"
    "}"
};

const FIXED: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "use std::collections::HashMap;"
    ""
    "pub fn present_and_true(verbose: Option<bool>) -> bool {"
    "    verbose.unwrap_or(false)"
    "}"
    ""
    "pub fn present_and_false(verbose: Option<bool>) -> bool {"
    "    !verbose.unwrap_or(true)"
    "}"
    ""
    r#"pub fn borrowed_payload(flags: &HashMap<String, bool>) -> bool {"#
    r#"    flags.get("verbose").copied().unwrap_or(false)"#
    "}"
    ""
    "pub fn borrowed_option(enabled: &Option<bool>) -> bool {"
    "    enabled.unwrap_or(false)"
    "}"
    ""
    "pub fn dereferenced_option(enabled: &Option<bool>) -> bool {"
    "    (*enabled).unwrap_or(false)"
    "}"
    ""
    "pub fn under_an_and(verbose: Option<bool>, other: bool) -> bool {"
    "    other && !verbose.unwrap_or(false)"
    "}"
};

/// Run the fixer over the fixture and hand back what it left on disk,
/// plus its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        "some_bool_comparison_autofix",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE)],
        CONFIG,
    );
    let (stderr, success) = run_dylint_fix(temp.path(), &shared_target_dir());
    assert!(
        success,
        "`cargo dylint --fix` failed; stderr was:\n{stderr}",
    );
    let fixed = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read fixed fixture");
    (temp, fixed, stderr)
}

#[test]
fn the_autofix_leaves_the_crate_compiling() {
    let (_temp, fixed, stderr) = fix();

    // The headline assertion. `cargo fix` prints this after applying a
    // suggestion that does not compile, having reverted the file -- so
    // its absence is what says every applied rewrite was sound.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would also leave the file untouched, which the assertion
    // above cannot tell from a rule that stood down. Comparing the whole
    // file pins every rewrite, down to the brackets.
    assert_eq!(
        fixed, FIXED,
        "the fixer did not turn the fixture into its expected form",
    );
}
