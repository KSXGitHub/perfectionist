//! Render the change a round made to a codebase as one unified diff.
//!
//! A round fixture stores what the round did rather than a second copy
//! of the tree, so it grows with what the round changed rather than
//! with the size of the codebase. Nothing here applies a diff: the
//! harness diffs the tree the round actually produced against the one
//! it started from and compares that text against the stored fixture,
//! so a stored diff is an expectation and never an input.
//!
//! The format is the unified diff `git apply` and `patch` read, with
//! `a/` and `b/` prefixes, so a stored fixture can be fed to either by
//! hand while debugging.

use crate::autofix::snapshot::Snapshot;
use core::fmt::Write as _;
use core::ops::Range;
use std::collections::BTreeSet;

/// Lines of unchanged context a hunk carries on each side of a change.
/// Three is what `diff -u` and `git diff` default to.
const CONTEXT: usize = 3;

/// What one line of a file became between the two snapshots.
enum Op<'a> {
    /// Present on both sides, so it is only ever context.
    Keep(&'a str),
    /// Present before the round and gone after it.
    Remove(&'a str),
    /// Absent before the round and present after it.
    Insert(&'a str),
}

/// Which of the two files a hunk header is counting.
#[derive(Clone, Copy)]
enum Side {
    /// The file as the round found it, which the header counts after
    /// its `-`.
    Before,
    /// The file as the round left it, counted after the `+`.
    After,
}

impl Op<'_> {
    /// Whether the line is on the given side, which is what that
    /// side of the hunk header counts and what decides its line
    /// numbers.
    fn on(&self, side: Side) -> bool {
        match side {
            Side::Before => matches!(self, Op::Keep(_) | Op::Remove(_)),
            Side::After => matches!(self, Op::Keep(_) | Op::Insert(_)),
        }
    }

    /// The marker column and the line, as the diff body spells them.
    fn render(&self) -> (char, &str) {
        match self {
            Op::Keep(line) => (' ', line),
            Op::Remove(line) => ('-', line),
            Op::Insert(line) => ('+', line),
        }
    }
}

/// Diff `before` against `after`, file by file, over the union of the
/// paths either side holds. An empty return says the round changed
/// nothing.
pub fn unified(before: &Snapshot, after: &Snapshot) -> String {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let mut out = String::new();
    for path in paths {
        out.push_str(&file_diff(path, before.get(path), after.get(path)));
    }
    out
}

/// The diff of one path, headers included. `None` on a side is the
/// file not existing there, which the header spells `/dev/null`.
fn file_diff(path: &str, before: Option<&String>, after: Option<&String>) -> String {
    if before == after {
        return String::new();
    }
    let before_lines: Vec<&str> = before.map_or("", String::as_str).lines().collect();
    let after_lines: Vec<&str> = after.map_or("", String::as_str).lines().collect();
    let ops = operations(&before_lines, &after_lines);
    let mut out = String::new();
    writeln!(out, "--- {}", side("a", path, before.is_some())).expect("write a diff header");
    writeln!(out, "+++ {}", side("b", path, after.is_some())).expect("write a diff header");
    for hunk in hunks(&ops) {
        out.push_str(&render_hunk(&ops, hunk));
    }
    out
}

/// One header's path: `<prefix>/<path>` where the file exists on that
/// side, and `/dev/null` where it does not.
fn side(prefix: &str, path: &str, exists: bool) -> String {
    match exists {
        true => format!("{prefix}/{path}"),
        false => "/dev/null".to_owned(),
    }
}

/// Line up the two files line by line.
///
/// The lines are split by the caller rather than by `diff::lines`,
/// which splits on the separator and so hands back a trailing empty
/// line for every text that ends in one. Every fixture file does, the
/// snapshot reader holding each to it, so that line is an artefact of
/// the split and would render as a change wherever one side of a
/// path is absent. Splitting with `str::lines` drops it, and the same
/// assumption is what keeps `\ No newline at end of file` out of the
/// format.
fn operations<'a>(before: &'a [&'a str], after: &'a [&'a str]) -> Vec<Op<'a>> {
    diff::slice(before, after)
        .into_iter()
        .map(|line| match line {
            diff::Result::Left(line) => Op::Remove(line),
            diff::Result::Both(line, _) => Op::Keep(line),
            diff::Result::Right(line) => Op::Insert(line),
        })
        .collect()
}

/// Group the changed lines into hunks, each padded with [`CONTEXT`]
/// lines. Two changes closer than twice that padding land in one
/// hunk, which is what keeps the context from being printed twice.
fn hunks(ops: &[Op]) -> Vec<Range<usize>> {
    let mut hunks: Vec<Range<usize>> = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        if matches!(op, Op::Keep(_)) {
            continue;
        }
        let start = index.saturating_sub(CONTEXT);
        let end = ops.len().min(index + CONTEXT + 1);
        match hunks.last_mut() {
            Some(last) if last.end >= start => last.end = end,
            _ => hunks.push(start..end),
        }
    }
    hunks
}

/// One hunk: its `@@` header and its body.
fn render_hunk(ops: &[Op], hunk: Range<usize>) -> String {
    let before = span(ops, &hunk, Side::Before);
    let after = span(ops, &hunk, Side::After);
    let mut out = String::new();
    writeln!(
        out,
        "@@ -{},{} +{},{} @@",
        before.0, before.1, after.0, after.1,
    )
    .expect("write a hunk header");
    for op in &ops[hunk] {
        let (marker, line) = op.render();
        out.push(marker);
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The `(start, count)` pair one side of a hunk header carries.
/// `start` is 1-based, except that a hunk holding no line of that side
/// reports the line it comes after, which is what `diff -u` emits for
/// a file created or deleted whole.
fn span(ops: &[Op], hunk: &Range<usize>, side: Side) -> (usize, usize) {
    let preceding = ops[..hunk.start].iter().filter(|op| op.on(side)).count();
    let count = ops[hunk.clone()].iter().filter(|op| op.on(side)).count();
    let start = match count {
        0 => preceding,
        _ => preceding + 1,
    };
    (start, count)
}

#[cfg(test)]
mod tests;
