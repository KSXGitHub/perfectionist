//! Per-style compliance predicate over one run of consecutive `use`
//! statements. A `false` answer drives [`super::render`] to produce the
//! canonical replacement.

use super::UseStmt;
use super::config::Style;

/// Blank lines between two source-adjacent statements: the gap between
/// the end of `before` and the start of `after`, never negative.
pub(super) fn blanks_between(before: &UseStmt<'_>, after: &UseStmt<'_>) -> usize {
    after.start_line.saturating_sub(before.end_line + 1)
}

/// Whether `stmts` (in source order) already matches `style`.
pub(super) fn is_compliant(style: Style, blank_line_count: usize, stmts: &[UseStmt<'_>]) -> bool {
    match style {
        Style::SingleGroup => single_group_compliant(stmts),
        Style::Grouped => grouped_compliant(blank_line_count, stmts),
    }
}

/// `single_group`: no blank line may sit between any two statements in
/// the run.
fn single_group_compliant(stmts: &[UseStmt<'_>]) -> bool {
    stmts
        .windows(2)
        .all(|pair| blanks_between(&pair[0], &pair[1]) == 0)
}

/// `grouped`: ranks are non-decreasing in the configured order;
/// statements sharing a rank carry no blank line between them; a step
/// up to a later group carries exactly `blank_line_count` blank lines.
fn grouped_compliant(blank_line_count: usize, stmts: &[UseStmt<'_>]) -> bool {
    stmts.windows(2).all(|pair| {
        let blanks = blanks_between(&pair[0], &pair[1]);
        match pair[0].rank.cmp(&pair[1].rank) {
            std::cmp::Ordering::Equal => blanks == 0,
            std::cmp::Ordering::Less => blanks == blank_line_count,
            std::cmp::Ordering::Greater => false,
        }
    })
}
