//! End-to-end proof that `macro_trailing_comma`'s suggestions are
//! review-only: `cargo dylint --fix` must report both of them and
//! apply neither.
//!
//! ## What this actually tests
//!
//! Applicability, which the `ui/` sweep cannot reach: a `.stderr`
//! renders a `MaybeIncorrect` suggestion and a `MachineApplicable` one
//! identically, and applicability is the whole of what decides whether
//! a fixer rewrites the file. Running the fixer is the only way to ask.
//!
//! ## Why the suggestions are review-only
//!
//! rustfmt reformats a macro invocation whose arguments parse as an
//! expression list, and for a paren-delimited one it honours the
//! trailing comma it finds there rather than normalising it. So an
//! inserted comma survives into the single line rustfmt collapses the
//! invocation to, where this same lint reports the opposite violation:
//! a vertical `format!` fixed to carry a trailing comma reformats to
//! `format!("{} {}", 1, 2,)`, which is then reported as a single-line
//! invocation that should not end with one. Under the compact layout
//! rustfmt reshapes a bracket-delimited invocation to when its sole
//! element is multi-line, the inserted comma is dropped outright.
//!
//! Each exchange settles, because rustfmt is idempotent and the second
//! fix lands on a form it leaves alone. What the applicability buys is
//! therefore not termination but the round of reverse-direction churn
//! a silent fixer would otherwise spend unseen.
//!
//! The removal is held to review for symmetry rather than because it
//! moves anything, so the fixture carries it only to pin that the
//! fixer declines both shapes rather than just the insertion.

pub mod _utils;

use _utils::{TempDir, build_project, cargo_manifest_dir, run_dylint_fix, shared_target_dir};
use std::fs;
use text_block_macros::text_block_fnl;

/// The insertion shape is written vertically with its single-line form
/// well inside rustfmt's width, which is what makes rustfmt collapse it
/// and carry an inserted comma along. The removal shape is a
/// single-line invocation that already ends with one.
const SOURCE: &str = text_block_fnl! {
    "pub fn insertion() -> String {"
    "    format!("
    r#"        "{} {}","#
    "        1,"
    "        2"
    "    )"
    "}"
    ""
    "pub fn removal() -> Vec<u32> {"
    "    vec![1, 2, 3,]"
    "}"
};

const INSERTION_MESSAGE: &str = "multi-line macro invocation should end with a trailing comma";

const REMOVAL_MESSAGE: &str = "single-line macro invocation should not end with a trailing comma";

/// Run the fixer over the fixture and hand back what it left on disk,
/// plus its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project(
        temp.path(),
        "macro_trailing_comma_applicability",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE)],
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
fn the_fixer_reports_both_suggestions_and_applies_neither() {
    let (_temp, fixed, stderr) = fix();

    // A rule that stopped firing would also leave the file untouched,
    // which is the one way the headline assertion can pass for the
    // wrong reason. Both messages have to be in the fixer's own run,
    // not merely in some earlier `cargo dylint`.
    for message in [INSERTION_MESSAGE, REMOVAL_MESSAGE] {
        assert!(
            stderr.contains(message),
            "the fixer's run did not report {message:?}; stderr was:\n{stderr}",
        );
    }

    // The headline assertion: a `MachineApplicable` suggestion here
    // would have been applied, so a byte-identical file is what says
    // both are still `MaybeIncorrect`.
    assert_eq!(
        fixed, SOURCE,
        "the fixer rewrote the fixture, so a suggestion is machine-applicable again",
    );
}
