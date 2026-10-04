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
use text_block_macros::text_block_fnl;

/// The chain and guard-and-value triggers describe their splits rather
/// than rewriting them, and `overly_long_method_chain` would count the
/// adapters the fix adds.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["overly_long_method_chain"]"#
};

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
        "`cargo dylint --fix` failed; stderr was:\n{stderr}"
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
