//! End-to-end proof of which rewrites `mutating_command_builder` hands
//! the fixer, and which it declines.
//!
//! Applicability is the one property a `.stderr` cannot show: a
//! suggestion shown as a concrete rewrite looks the same whether or not
//! the fixer will apply it. So the fixtures are run through the real
//! fixer and judged on what it did to the source.
//!
//! What makes a rewrite applicable here is that it preserves the
//! expression's type. The rename alone turns `&mut Command` into
//! `Command`, which the context may reject, and which can redirect a
//! later call in the same chain to an extension trait of the author's
//! own -- compiling, and invisible in the diff. Prefixing `&mut `
//! restores the type, so the context and every following method resolve
//! as they did before. What is left after that is checkable, and the
//! lint's own rustdoc lists it.
//!
//! A chain moves whole or not at all, and `shadowed_next_link` is why.
//! Renaming only the head would leave the rest calling std's setters
//! against a receiver that is now owned, which is how a by-value method
//! of the author's own comes to be found before the inherent one.
//!
//! The fixtures split by direction, because the two fail differently.
//! `applied.rs` is compared against `applied.fixed.rs`, so a rewrite
//! that stops being applied, or starts being applied differently, fails
//! here. `not_applied.rs` is compared against itself, and most of its
//! shapes would compile if they were rewritten anyway, so that
//! comparison can fail rather than passing because `cargo fix` reverted
//! the file.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, fixture_cargo_toml, run_dylint_fix,
    shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// The generated manifest with `command-extra` appended, which the
/// fixtures need and [`fixture_cargo_toml`] does not carry. Passing
/// `Cargo.toml` as a source overwrites the generated copy, since
/// [`build_project_with_config`] inserts the sources after its own
/// entries — appending to that copy rather than restating it keeps the
/// package, lib and workspace stanzas in one place.
fn cargo_toml(package: &str) -> String {
    format!(
        "{}\n[dependencies]\ncommand-extra = \"1.2.0\"\n",
        fixture_cargo_toml(package),
    )
}

/// The shapes the fixer is asserted to rewrite, and what it must turn
/// them into. They live in files rather than in literals here: what the
/// assertion compares is what a reader edits.
const APPLIED: &str = include_str!("fixtures/mutating_command_builder_autofix/applied.rs");

const APPLIED_FIXED: &str =
    include_str!("fixtures/mutating_command_builder_autofix/applied.fixed.rs");

/// The shapes the rule declines to hand over, for each of the reasons
/// it declines.
const NOT_APPLIED: &str = include_str!("fixtures/mutating_command_builder_autofix/not_applied.rs");

/// Sibling rules would rewrite the same lines on their own account,
/// which would make "did the fixer touch this line?" answer the wrong
/// question -- and any error one of them introduced would be blamed on
/// this rule by the headline assertion below.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["bare_identifier_reference", "impure_macro_arguments", "import_granularity_mismatch", "import_grouping_mismatch"]"#
};

/// Run the fixer over one fixture crate and hand back what it left on
/// disk, plus its stderr.
fn fix(package: &str, source: &str) -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        package,
        cargo_manifest_dir(),
        &[("Cargo.toml", &cargo_toml(package)), ("src/lib.rs", source)],
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
#[ignore = "builds the lint and resolves `command-extra` from the registry in a fresh fixture crate"]
fn the_fixer_applies_the_whole_rewrite() {
    let (_temp, fixed, stderr) = fix("mutating_command_builder_applied", APPLIED);

    // `cargo fix` prints this after applying a suggestion that does not
    // compile, having reverted the file. A rewrite this rule hands over
    // is one it has established compiles, so it should never appear.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    assert_eq!(
        fixed, APPLIED_FIXED,
        "the fixer did not turn the fixture into its `.fixed` counterpart",
    );
}

#[test]
#[ignore = "builds the lint and resolves `command-extra` from the registry in a fresh fixture crate"]
fn the_fixer_declines_the_rest() {
    let (_temp, fixed, stderr) = fix("mutating_command_builder_not_applied", NOT_APPLIED);

    assert!(
        !stderr.contains("errors present after applying fixes"),
        "nothing here should have been applied, let alone reverted; stderr was:\n{stderr}",
    );

    assert_eq!(
        fixed, NOT_APPLIED,
        "the fixer rewrote a shape the rule declined to hand it",
    );

    // And the rule fired on each shape, so the assertion above is not
    // passing because the fixture went quiet.
    for shape in [
        "over-a-binding",
        "turbofish",
        "trait-out-of-scope",
        "ordered-drop",
        "macro-argument",
        "shadowed-trailing-call",
    ] {
        assert!(
            stderr.contains(shape),
            "expected the rule to fire on `{shape}`; stderr was:\n{stderr}",
        );
    }
}
