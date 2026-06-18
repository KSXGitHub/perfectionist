//! Re-rendering one run of `use` statements into its canonical shape.
//!
//! Each statement's verbatim source text (attributes, visibility, and
//! the `use ...;` itself) is reproduced unchanged; the rule only moves
//! statements between groups and rewrites the blank lines between them.
//! Inner ordering within a group is left to `cargo fmt`, so the
//! relative order of statements that share a group is preserved (the
//! sort is stable).

use super::UseStmt;

/// Build the replacement text for the whole run. `pad` is the run's
/// indentation, prepended to every line after the first (the first
/// line's indent is left in the source, outside the replaced span).
///
/// The run is stable-partitioned by group rank (assigned in
/// [`super::classify::rank`]); order within a rank stands, so a
/// `single_block` run with one rank keeps source order while a
/// `multi_block` run keeps `cargo fmt`'s inner order. Adjacent
/// statements are then separated by one blank line across a rank step
/// and none within a rank — which, for a one-rank `single_block` run,
/// collapses to one contiguous block.
pub(super) fn replacement(pad: &str, stmts: &[UseStmt<'_>]) -> String {
    let mut ordered: Vec<&UseStmt<'_>> = stmts.iter().collect();
    ordered.sort_by_key(|stmt| stmt.rank);

    let mut out = String::new();
    for (index, stmt) in ordered.iter().enumerate() {
        if index > 0 {
            // None within a rank; exactly one blank line across a step.
            let blanks = usize::from(ordered[index - 1].rank != stmt.rank);
            // One newline ends the previous statement; `blanks` more
            // produce that many empty lines; then the shared indent.
            for _ in 0..=blanks {
                out.push('\n');
            }
            out.push_str(pad);
        }
        out.push_str(&stmt.text);
    }
    out
}
