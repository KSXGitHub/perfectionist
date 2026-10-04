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
//! lands. The fixer answers the first two, and the oracle answers the
//! third: it asserts what every function returns, and is run once before
//! the fixer and once after, so a rewrite that compiles and answers
//! differently fails the second run. Comparing the fixed file against its
//! expected text cannot catch that, text being all it reads — and that
//! comparison goes green again the moment someone updates the expected
//! text to whatever the rewrite now emits, which is the drift the oracle
//! is here for.
//!
//! `cargo dylint --fix` rewrites `--lib` only, so the oracle is not among
//! what the fixer may edit, and cannot drift toward its output.
//!
//! The fixture, its expected form and the oracle are files under
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
    TempDir, build_project_with_config, cargo_manifest_dir, run_cargo_test, run_dylint_fix,
    shared_target_dir,
};
use std::fs;
use std::sync::LazyLock;

/// No rule is disabled, so the fixed form is held to every rule this
/// crate ships. `overly_long_method_chain` is the one the added adapters
/// could trip, and does not: the longest chain the fixer leaves runs to
/// four calls against that rule's default of five.
const CONFIG: &str = "";

const SOURCE: &str = include_str!("fixtures/splittable_adapter_closure_autofix/applied.rs");

const FIXED: &str = include_str!("fixtures/splittable_adapter_closure_autofix/applied.fixed.rs");

/// What every function in the fixture answers, asserted inside the fixture
/// so it runs against the folded form and the split one alike.
///
/// Each input is chosen to tell the split apart from a plausible wrong one.
/// `takes_a_prefix` is asked twice, because one lift target per conjunct
/// can be wrong on its own: a line failing the second test before a passing
/// one catches the last adapter spliced as `filter`, and a line failing the
/// *first* test before a passing one catches the lifted one, which every
/// line passing the first test would hide. `holds_a_disjunction` has a line
/// passing on each side of the `||`, and one passing neither.
/// `moves_a_capture` is asked once where a line satisfies both tests and
/// once where one line satisfies each, which is the case a conjunction
/// answers `false` and two independent `any` calls would answer `true`.
const ORACLE: &str = include_str!("fixtures/splittable_adapter_closure_autofix/oracle.rs");

/// What one fixer run produces, read out before the fixture is dropped.
struct Run {
    /// Whether the oracle holds for the folded form, and its output.
    before: (String, bool),
    /// What the fixer left in `src/lib.rs`.
    fixed: String,
    /// The fixer's own stderr, which names a rewrite it had to revert.
    fix_stderr: String,
    /// Whether the oracle holds for the split form, and its output.
    after: (String, bool),
}

/// One run, shared by every test below.
///
/// Each test asserts one property of it, and a test per property reports
/// which one failed. Running the fixer per test instead would pay for the
/// fixture once per test to learn the same things.
static RUN: LazyLock<Run> = LazyLock::new(run);

/// Materialise the fixture, run its oracle, run the fixer, then run the
/// oracle again over what the fixer left.
fn run() -> Run {
    let temp = TempDir::new().expect("failed to create temp dir");
    let target = shared_target_dir();
    build_project_with_config(
        temp.path(),
        "splittable_adapter_closure_autofix",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE), ("tests/it.rs", ORACLE)],
        CONFIG,
    );
    let before = run_cargo_test(temp.path(), &target);
    let (fix_stderr, success) = run_dylint_fix(temp.path(), &target);
    assert!(
        success,
        "`cargo dylint --fix` failed; stderr was:\n{fix_stderr}",
    );
    let fixed = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read fixed fixture");
    let after = run_cargo_test(temp.path(), &target);
    Run {
        before,
        fixed,
        fix_stderr,
        after,
    }
}

#[test]
fn the_oracle_holds_for_the_folded_form() {
    // Asserted on its own, because an oracle wrong about the form the rule
    // was given cannot say anything about the form it produced.
    let (output, passed) = &RUN.before;
    assert!(
        *passed,
        "the oracle does not hold for the folded form, so it cannot judge \
         the split one; output was:\n{output}",
    );
}

#[test]
fn the_autofix_leaves_the_crate_compiling() {
    // `cargo fix` prints this after applying a suggestion that does not
    // compile, having reverted the file, so its absence is what says every
    // applied rewrite was sound.
    let stderr = &RUN.fix_stderr;
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would also leave the file untouched, which the assertion
    // above cannot tell from a rule that stood down. Comparing the whole
    // file pins every rewrite, down to the brackets, and holds the
    // commented predicate to being left alone.
    assert_eq!(
        RUN.fixed, FIXED,
        "the fixer did not turn the fixture into its expected form",
    );
}

#[test]
fn the_autofix_keeps_every_answer() {
    // The oracle held for the folded form, so holding for what the fixer
    // left is the rewrite answering the same. Compiling is not that: every
    // defect this test was written for compiled.
    let (output, passed) = &RUN.after;
    assert!(
        *passed,
        "the split form does not answer what the folded form did; output \
         was:\n{output}",
    );
}

#[test]
fn the_autofix_leaves_nothing_for_a_second_pass() {
    // A rewrite the rule fires on again would have the reader apply a fix
    // per pass, so the fixed form has to be a fixed point. The commented
    // predicate is the exception the rule declines to rewrite, and stays
    // reported, which is why this counts rather than asking for silence.
    // `cargo fix` reports the diagnostics of its final check pass, which is
    // over the fixed form, and not those of the passes that did the fixing.
    // So the fixer's own stderr is the second pass, and asking for one more
    // `cargo dylint` run would buy nothing. If that ever stopped holding,
    // the intermediate reports would push this count above one rather than
    // hide anything.
    let stderr = &RUN.fix_stderr;
    let reported = stderr.matches("runs 2 tests on the item").count()
        + stderr.matches("runs 3 tests on the item").count();
    assert_eq!(
        reported, 1,
        "the fixed form should leave only the commented predicate \
         reported; stderr was:\n{stderr}",
    );

    // The diagnostic quotes the offending line rather than naming the
    // function it sits in, and the comment is what that line has that no
    // other predicate in the fixture does.
    assert!(
        stderr.contains("/* both */"),
        "the one remaining report should be the commented predicate; \
         stderr was:\n{stderr}",
    );
}
