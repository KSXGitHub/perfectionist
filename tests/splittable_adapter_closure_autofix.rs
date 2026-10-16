//! End-to-end proof that `splittable_adapter_closure`'s rewrite compiles
//! and keeps the answer where the rule hands it over as
//! `MachineApplicable`.
//!
//! ## What this actually tests
//!
//! Not the suggestion text, which the `ui/` sweep pins: a `.stderr` cannot
//! show applicability at all, and that is the property deciding whether
//! `cargo dylint --fix` touches a suggestion. What is left is whether the
//! text the rule splices parses, type-checks and answers the same where it
//! lands, and the fixer answers the first two while the assertions below
//! answer the third.
//!
//! ## What it does not test
//!
//! That the split *answers* what the folded form did. Text is all the
//! comparison reads, so a rewrite whose expected form is updated to match it
//! passes however it behaves. What catches that is an oracle inside the
//! fixture: assertions about what each function returns, run before the
//! fixer and again after. Measured, it cost two extra `cargo test`
//! invocations, about 0.75s for this rule alone, and one such fixture per
//! rule with an autofix is the part that does not scale. The mechanism that
//! would make it scale, one project holding every rule's inputs, fixed in
//! one pass, with the result cached, is not designed yet, so the property is
//! left to that design rather than paid for per rule.
//!
//! The fixture and its expected form are files under
//! `tests/fixtures/splittable_adapter_closure_autofix/`, where a `.rs`
//! fixture is read and edited as Rust rather than as a quoted string.
//!
//! The shapes that earned the test are each a way the rewrite could compile
//! as something other than the conjunction it replaced:
//!
//! - Three tests rather than two, so the loop that lifts all but the last
//!   is exercised past one iteration.
//! - A conjunct that is a disjunction, whose own brackets have to survive
//!   or the `||` would bind across the `&&` boundary the split removed.
//! - A `move` closure, where every closure the split writes needs the
//!   keyword or the capture is borrowed rather than copied.
//! - An adapter that is not `filter`: the last test keeps the adapter the
//!   folded form had, and `any` returning `bool` is the one that would
//!   show a `filter` spliced in its place.
//! - A prefix-shaped adapter, whose tests lift into `take_while` rather
//!   than `filter`, so the lift target is read from the discipline.
//! - An `Option` receiver, where the lift target is `Option::filter`.
//! - A predicate holding a comment, which is offered as advice rather than
//!   applied, so the fixer must leave it alone.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, run_dylint_fix, shared_target_dir,
};
use std::fs;

/// No rule is disabled, so the fixed form is held to every rule this crate
/// ships. `overly_long_method_chain` is the one the added adapters could
/// trip, and does not: the longest chain the fixer leaves runs to four calls
/// against that rule's default of five.
const CONFIG: &str = "";

const SOURCE: &str = include_str!("fixtures/splittable_adapter_closure_autofix/applied.rs");

const FIXED: &str = include_str!("fixtures/splittable_adapter_closure_autofix/applied.fixed.rs");

/// Run the fixer over the fixture and hand back what it left on disk, plus
/// its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        "splittable_adapter_closure_autofix",
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
    // suggestion that does not compile, having reverted the file, so its
    // absence is what says every applied rewrite was sound.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would also leave the file untouched, which the assertion
    // above cannot tell from a rule that stood down. Comparing the whole
    // file pins every rewrite, down to the brackets, and holds the
    // commented predicate to being left alone.
    assert_eq!(
        fixed, FIXED,
        "the fixer did not turn the fixture into its expected form",
    );
}
