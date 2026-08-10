//! Per-style compliance predicate over one run of consecutive `use`
//! statements. A `false` answer drives [`super::render`] to produce the
//! canonical replacement.

use super::UseStmt;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

/// Number of whitespace-only lines strictly between two source-adjacent
/// statements, given the source text of the gap between them (from the
/// end of the first statement to the start of the second).
///
/// Only genuinely blank lines count as a separator. A line comment
/// (`// ...`) carries non-whitespace text, so it is already not blank; a
/// whitespace-only line *inside* a `/* ... */` block comment is part of
/// the comment, not a separator, so block-comment interiors are excluded
/// explicitly. The first and last pieces of the split are the trailing /
/// leading remainders of the two statements' own lines, never lines of
/// the gap itself.
pub(super) fn count_blank_lines(gap: &str) -> usize {
    let lines: Vec<&str> = gap.split('\n').collect();
    if lines.len() <= 2 {
        return 0;
    }
    let block_comments = block_comment_ranges(gap);
    let mut count = 0;
    let mut offset = 0;
    for (index, line) in lines.iter().enumerate() {
        let line_start = offset;
        // `+ 1` for the `\n` that `split` consumed between lines.
        offset += line.len() + 1;
        if index == 0 || index == lines.len() - 1 {
            continue;
        }
        if !line.trim().is_empty() {
            continue;
        }
        let inside_block_comment = block_comments
            .iter()
            .any(|&(start, end)| line_start >= start && line_start < end);
        if !inside_block_comment {
            count += 1;
        }
    }
    count
}

/// Byte ranges of every `/* ... */` block comment in `text`, so
/// [`count_blank_lines`] can tell a real blank line from a
/// whitespace-only line inside a multi-line block comment.
fn block_comment_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    for token in tokenize(text, FrontmatterAllowed::No) {
        let length = token.len as usize;
        if matches!(token.kind, TokenKind::BlockComment { .. }) {
            ranges.push((offset, offset + length));
        }
        offset += length;
    }
    ranges
}

/// Whether `stmts` (in source order) already matches the configured
/// style. `blanks` holds the blank-line count between each adjacent pair
/// (so its length is `stmts.len() - 1`), as counted by
/// [`count_blank_lines`].
///
/// Both styles reduce to one rule once each statement carries its group
/// rank (assigned in [`super::classify::rank`]): ranks are non-decreasing
/// down the run; statements sharing a rank carry no blank line between
/// them; a step up to a later group carries exactly one blank line. The
/// style lives entirely in the ranks — `single_block` gives every import
/// rank `0` (so the rule degenerates to "no blank lines anywhere"), bar an
/// opt-in trailing cfg block at a higher rank; `multi_block` assigns the
/// std / internal / third-party / cfg ranks. Orthogonally, the
/// `reexports` knob reserves the leading rank(s) for re-exports — rank
/// `0` under `grouped`, ranks `0` and `1` under `split` (submodule then
/// alias) — and shifts every other rank down past them, so a run that
/// mixes re-exports with private imports carries one blank line between
/// the two — even under `single_block`.
pub(super) fn is_compliant(stmts: &[UseStmt<'_>], blanks: &[usize]) -> bool {
    stmts
        .windows(2)
        .zip(blanks)
        .all(|(pair, &blanks)| match pair[0].rank.cmp(&pair[1].rank) {
            core::cmp::Ordering::Equal => blanks == 0,
            core::cmp::Ordering::Less => blanks == 1,
            core::cmp::Ordering::Greater => false,
        })
}
