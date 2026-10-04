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
//! lands. The fixer answers the first two, and `tests/it.rs` inside the
//! fixture answers the third: it asserts what every function returns, and
//! is run once before the fixer and once after, so a rewrite that compiles
//! and answers differently fails the second run. Comparing the fixed file
//! against its expected text cannot catch that, text being all it reads.
//!
//! `cargo dylint --fix` rewrites `--lib` only, so the assertions are not
//! among what the fixer may edit, and the oracle cannot drift toward
//! whatever the rewrite happened to produce.
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
    TempDir, build_project_with_config, cargo_manifest_dir, run_cargo_test, run_dylint,
    run_dylint_fix, shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// No rule is disabled, so the fixed form is held to every rule this
/// crate ships. `overly_long_method_chain` is the one the added adapters
/// could trip, and does not: the longest chain the fixer leaves runs to
/// four calls against that rule's default of five.
const CONFIG: &str = "";

const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "pub fn wanted(line: &str) -> bool {"
    "    !line.is_empty()"
    "}"
    ""
    "pub fn longer(line: &str, limit: usize) -> bool {"
    "    line.len() > limit"
    "}"
    ""
    "pub fn shorter(line: &str, limit: usize) -> bool {"
    "    line.len() < limit * 2"
    "}"
    ""
    "pub fn three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    r#"    lines.filter(|line| wanted(line) && line.starts_with('#') && line.ends_with('!')).collect()"#
    "}"
    ""
    "pub fn holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    r#"    lines.filter(|line| wanted(line) && (line.starts_with('#') || line.ends_with('!'))).collect()"#
    "}"
    ""
    "pub fn moves_a_capture(limit: usize, mut lines: std::vec::IntoIter<&'static str>) -> bool {"
    "    lines.any(move |line| longer(line, limit) && shorter(line, limit))"
    "}"
    ""
    "pub fn takes_a_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    r#"    lines.take_while(|line| wanted(line) && line.starts_with('#')).collect()"#
    "}"
    ""
    "pub fn an_option(line: Option<&'static str>) -> bool {"
    r#"    line.is_some_and(|line| wanted(line) && line.starts_with('#'))"#
    "}"
    ""
    "pub fn holds_a_comment(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    "    lines"
    r#"        .filter(|line| wanted(line) /* both */ && line.starts_with('#'))"#
    "        .collect()"
    "}"
};

const FIXED: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "pub fn wanted(line: &str) -> bool {"
    "    !line.is_empty()"
    "}"
    ""
    "pub fn longer(line: &str, limit: usize) -> bool {"
    "    line.len() > limit"
    "}"
    ""
    "pub fn shorter(line: &str, limit: usize) -> bool {"
    "    line.len() < limit * 2"
    "}"
    ""
    "pub fn three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    r#"    lines.filter(|line| wanted(line)).filter(|line| line.starts_with('#')).filter(|line| line.ends_with('!')).collect()"#
    "}"
    ""
    "pub fn holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    r#"    lines.filter(|line| wanted(line)).filter(|line| (line.starts_with('#') || line.ends_with('!'))).collect()"#
    "}"
    ""
    "pub fn moves_a_capture(limit: usize, mut lines: std::vec::IntoIter<&'static str>) -> bool {"
    "    lines.filter(move |line| longer(line, limit)).any(move |line| shorter(line, limit))"
    "}"
    ""
    "pub fn takes_a_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    r#"    lines.take_while(|line| wanted(line)).take_while(|line| line.starts_with('#')).collect()"#
    "}"
    ""
    "pub fn an_option(line: Option<&'static str>) -> bool {"
    r#"    line.filter(|line| wanted(line)).is_some_and(|line| line.starts_with('#'))"#
    "}"
    ""
    "pub fn holds_a_comment(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {"
    "    lines"
    r#"        .filter(|line| wanted(line) /* both */ && line.starts_with('#'))"#
    "        .collect()"
    "}"
};

/// The oracle: what every function in the fixture answers, asserted
/// inside the fixture so it runs against the folded form and the split one
/// alike.
///
/// Each input is chosen to tell the split apart from a plausible wrong
/// one. `takes_a_prefix` is asked twice, because one lift target per
/// conjunct can be wrong on its own: a line failing the second test before
/// a passing one catches the last adapter spliced as `filter`, and a line
/// failing the *first* test before a passing one catches the lifted one,
/// which every line passing the first test would hide. `holds_a_disjunction` has a line passing on each side of
/// the `||`, and one passing neither. `moves_a_capture` is asked once
/// where a line satisfies both tests and once where one line satisfies
/// each, which is the case a conjunction answers `false` and two
/// independent `any` calls would answer `true`.
const ORACLE: &str = text_block_fnl! {
    "use splittable_adapter_closure_autofix::*;"
    ""
    "#[test]"
    "fn three_tests_keeps_the_lines_passing_all_three() {"
    r##"    let lines = vec!["#a!", "#b", "c!", "", "#c!"];"##
    r##"    assert_eq!(three_tests(lines.into_iter()), vec!["#a!", "#c!"]);"##
    "}"
    ""
    "#[test]"
    "fn holds_a_disjunction_keeps_either_alternative() {"
    r##"    let lines = vec!["#x", "y!", "", "z"];"##
    r##"    assert_eq!(holds_a_disjunction(lines.into_iter()), vec!["#x", "y!"]);"##
    "}"
    ""
    "#[test]"
    "fn moves_a_capture_wants_one_line_passing_both() {"
    r#"    let both = vec!["ab", "abcdefgh", "abcd"];"#
    "    assert!(moves_a_capture(3, both.into_iter()));"
    r#"    let one_each = vec!["ab", "abcdefgh"];"#
    "    assert!(!moves_a_capture(3, one_each.into_iter()));"
    "}"
    ""
    "#[test]"
    "fn takes_a_prefix_stops_at_the_first_failing_line() {"
    r##"    let stopped_by_the_second_test = vec!["#a", "#b", "c", "#d"];"##
    "    assert_eq!("
    "        takes_a_prefix(stopped_by_the_second_test.into_iter()),"
    r##"        vec!["#a", "#b"],"##
    "    );"
    r##"    let stopped_by_the_first_test = vec!["#a", "", "#c"];"##
    "    assert_eq!("
    "        takes_a_prefix(stopped_by_the_first_test.into_iter()),"
    r##"        vec!["#a"],"##
    "    );"
    "}"
    ""
    "#[test]"
    "fn an_option_wants_a_present_non_empty_hash() {"
    r##"    assert!(an_option(Some("#a")));"##
    r#"    assert!(!an_option(Some("a")));"#
    r#"    assert!(!an_option(Some("")));"#
    "    assert!(!an_option(None));"
    "}"
    ""
    "#[test]"
    "fn holds_a_comment_is_unchanged_by_the_fixer() {"
    r##"    let lines = vec!["#a", "b", ""];"##
    r##"    assert_eq!(holds_a_comment(lines.into_iter()), vec!["#a"]);"##
    "}"
};

/// Materialise the fixture, run its oracle, run the fixer, and hand back
/// what it left on disk plus its stderr.
///
/// The oracle runs before the fixer so a failure there is read as the
/// assertions being wrong about the folded form, rather than as the rewrite
/// having broken something.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        "splittable_adapter_closure_autofix",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE), ("tests/it.rs", ORACLE)],
        CONFIG,
    );
    let (before, passed) = run_cargo_test(temp.path(), &shared_target_dir());
    assert!(
        passed,
        "the oracle does not hold for the folded form, so it cannot judge \
         the split one; output was:\n{before}",
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

#[test]
fn the_autofix_keeps_every_answer() {
    let (temp, _fixed, _stderr) = fix();

    // The oracle held for the folded form before the fixer ran, so holding
    // for what the fixer left is the rewrite answering the same. Compiling
    // is not that: every defect this test was written for compiled.
    let (after, passed) = run_cargo_test(temp.path(), &shared_target_dir());
    assert!(
        passed,
        "the split form does not answer what the folded form did; output \
         was:\n{after}",
    );
}

#[test]
fn the_autofix_leaves_nothing_for_a_second_pass() {
    let (temp, _fixed, _stderr) = fix();

    // A rewrite the rule fires on again would have the reader apply a fix
    // per pass, so the fixed form has to be a fixed point. The commented
    // predicate is the exception the rule declines to rewrite, and stays
    // reported, which is why this counts rather than asking for silence.
    let (stderr, _success) = run_dylint(temp.path(), &shared_target_dir());
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
