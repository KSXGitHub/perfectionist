//! End-to-end proof of which rewrites `folded_command_setter` hands the
//! fixer, and which it declines.
//!
//! Applicability is the one property a `.stderr` cannot show: a
//! suggestion shown as a concrete rewrite looks the same whether or not
//! the fixer will apply it. So the fixtures are run through the real
//! fixer and judged on what it did to the source.
//!
//! Most declines are a question of order. The plural takes the initial
//! value as its receiver and the folded iterator as its argument, so it
//! evaluates them in the order the fold did not, and only a receiver
//! that runs nothing but the standard library's code, and reads nothing
//! the initial value writes or moves, is known not to care.
//! `src/rules/folded_command_setter/receiver.rs` derives it.
//!
//! The fixtures split by direction, because the two fail differently.
//! `applied.rs` is compared against `applied.fixed.rs`, so a rewrite
//! that stops being applied, or starts being applied differently, fails
//! here. `not_applied.rs` is compared against itself. Where a shape
//! would compile if rewritten, that comparison is what fails; where it
//! would not, `cargo fix` reverts the file, and the check for errors
//! after applying fixes fails instead.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, fixture_cargo_toml, run_dylint_fix,
    shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// The generated manifest with `command-extra` and `dependencies`
/// appended, which the fixtures need and [`fixture_cargo_toml`] does not
/// carry. Passing `Cargo.toml` as a source overwrites the generated copy,
/// since [`build_project_with_config`] inserts the sources after its own
/// entries. Appending to that copy rather than restating it keeps the
/// package, lib and workspace stanzas in one place.
fn cargo_toml(package: &str, dependencies: &str) -> String {
    format!(
        "{}\n[dependencies]\ncommand-extra = \"1.2.0\"\n{dependencies}",
        fixture_cargo_toml(package),
    )
}

/// A second crate compiled as `command_extra`, imported as `fake`, whose
/// `CommandExtra` declares no plural. `not_applied.rs` imports it where a
/// fold names the published trait by path.
const FAKE_DEPENDENCY: &str = "fake = { package = \"fake-command-extra\", path = \"fake\" }\n";

const FAKE_MANIFEST: &str = text_block_fnl! {
    "[package]"
    r#"name = "fake-command-extra""#
    r#"version = "0.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"name = "command_extra""#
    r#"path = "src/lib.rs""#
};

const FAKE_LIB: &str = text_block_fnl! {
    "pub trait CommandExtra: Sized {"
    "    fn without_env(self, key: impl AsRef<std::ffi::OsStr>) -> Self;"
    "}"
};

/// The shapes the fixer is asserted to rewrite, and what it must turn
/// them into. They live in files rather than in literals here: what the
/// assertion compares is what a reader edits.
const APPLIED: &str = include_str!("fixtures/folded_command_setter_autofix/applied.rs");

const APPLIED_FIXED: &str = include_str!("fixtures/folded_command_setter_autofix/applied.fixed.rs");

/// The shapes the rule declines to hand over, for each of the reasons
/// it declines.
const NOT_APPLIED: &str = include_str!("fixtures/folded_command_setter_autofix/not_applied.rs");

/// Sibling rules would rewrite the same lines on their own account, so a
/// changed line would no longer be this rule's doing, and an error one
/// of them introduced would be blamed on this rule.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["bare_identifier_reference", "impure_macro_arguments", "import_granularity_mismatch", "import_grouping_mismatch"]"#
};

/// Run the fixer over one fixture crate, beside `extra` files and with
/// `dependencies`, and hand back what it left on disk, plus its stderr.
fn fix(
    package: &str,
    source: &str,
    dependencies: &str,
    extra: &[(&str, &str)],
) -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let manifest = cargo_toml(package, dependencies);
    let mut sources = vec![("Cargo.toml", manifest.as_str()), ("src/lib.rs", source)];
    sources.extend_from_slice(extra);
    build_project_with_config(temp.path(), package, cargo_manifest_dir(), &sources, CONFIG);
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
fn the_fixer_replaces_the_fold_with_the_plural() {
    let (_temp, fixed, stderr) = fix("folded_command_setter_applied", APPLIED, "", &[]);

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
fn the_fixer_declines_the_reorderings_it_cannot_vouch_for() {
    let (_temp, fixed, stderr) = fix(
        "folded_command_setter_not_applied",
        NOT_APPLIED,
        FAKE_DEPENDENCY,
        &[
            ("fake/Cargo.toml", FAKE_MANIFEST),
            ("fake/src/lib.rs", FAKE_LIB),
        ],
    );

    assert!(
        !stderr.contains("errors present after applying fixes"),
        "nothing here should have been applied, let alone reverted; stderr was:\n{stderr}",
    );

    assert_eq!(
        fixed, NOT_APPLIED,
        "the fixer rewrote a shape the rule declined to hand it",
    );

    // The rule fired on each shape, so the fixture did not pass by going
    // quiet.
    for shape in [
        "local-iter",
        "shadowed-into-iter",
        "mutating-receiver",
        "user-deref",
        "explicit-deref",
        "written-place",
        "static-mut-place",
        "assigned-place",
        "written-in-closure",
        "deref-under-field",
        "pinned-deref",
        "lazy-lock",
        "interior-mutable",
        "user-into-iter",
        "user-item-clone",
        "panicking-receiver",
        "moved-root",
        "initial_is_the_root",
        "aliased-copy",
        "aliased-static-items",
        "raw-copy",
        "inferred-initial",
        "inferred_binding",
        "inferred_closure_parameter",
        "inferred_let_pattern",
        "inferred_match_arm",
        "diverging_branch",
        "item-annotated",
        "turbofish",
        "dropped-comment",
        "OVERRIDDEN_VARS",
        "AMBIGUOUS_VARS",
        "NOT_IN_SCOPE",
        "OTHER_TRAIT",
    ] {
        assert!(
            stderr.contains(shape),
            "expected the rule to fire on `{shape}`; stderr was:\n{stderr}",
        );
    }
}
