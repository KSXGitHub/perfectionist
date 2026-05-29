//! Per-style compliance predicate over one run of consecutive `use`
//! statements. A `false` answer drives [`super::render`] to produce the
//! canonical replacement.

use super::UseStmt;
use super::config::Style;

/// Number of whitespace-only lines strictly between two source-adjacent
/// statements, given the source text of the gap between them (from the
/// end of the first statement to the start of the second).
///
/// Only genuinely blank lines count as a separator: a comment line
/// (`// ...` or `/* ... */`) between two imports is content, not a blank
/// line, so it must not be read as a group boundary. The first and last
/// pieces of the split are the trailing / leading remainders of the two
/// statements' own lines, never lines of the gap itself.
pub(super) fn count_blank_lines(gap: &str) -> usize {
    let lines: Vec<&str> = gap.split('\n').collect();
    if lines.len() <= 2 {
        return 0;
    }
    lines[1..lines.len() - 1]
        .iter()
        .filter(|line| line.trim().is_empty())
        .count()
}

/// Whether `stmts` (in source order) already matches `style`. `blanks`
/// holds the blank-line count between each adjacent pair (so its length
/// is `stmts.len() - 1`), as counted by [`count_blank_lines`].
pub(super) fn is_compliant(
    style: Style,
    blank_line_count: usize,
    stmts: &[UseStmt<'_>],
    blanks: &[usize],
) -> bool {
    match style {
        Style::SingleGroup => single_group_compliant(blanks),
        Style::Grouped => grouped_compliant(blank_line_count, stmts, blanks),
    }
}

/// `single_group`: no blank line may sit between any two statements in
/// the run.
fn single_group_compliant(blanks: &[usize]) -> bool {
    blanks.iter().all(|&blanks| blanks == 0)
}

/// `grouped`: ranks are non-decreasing in the configured order;
/// statements sharing a rank carry no blank line between them; a step
/// up to a later group carries exactly `blank_line_count` blank lines.
fn grouped_compliant(blank_line_count: usize, stmts: &[UseStmt<'_>], blanks: &[usize]) -> bool {
    stmts
        .windows(2)
        .zip(blanks)
        .all(|(pair, &blanks)| match pair[0].rank.cmp(&pair[1].rank) {
            std::cmp::Ordering::Equal => blanks == 0,
            std::cmp::Ordering::Less => blanks == blank_line_count,
            std::cmp::Ordering::Greater => false,
        })
}
