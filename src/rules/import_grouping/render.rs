//! Re-rendering one run of `use` statements into its canonical shape.
//!
//! Each statement's verbatim source text (attributes, visibility, and
//! the `use ...;` itself) is reproduced unchanged; the rule only moves
//! statements between groups and rewrites the blank lines between them.
//! Inner ordering within a group is left to `cargo fmt`, so the
//! relative order of statements that share a group is preserved (the
//! sort is stable).

use super::UseStmt;
use super::config::Style;

/// Build the replacement text for the whole run. `pad` is the run's
/// indentation, prepended to every line after the first (the first
/// line's indent is left in the source, outside the replaced span).
pub(super) fn replacement(
    style: Style,
    blank_line_count: usize,
    pad: &str,
    stmts: &[UseStmt<'_>],
) -> String {
    let ordered: Vec<&UseStmt<'_>> = match style {
        // Keep source order; the fix only collapses blank lines.
        Style::SingleGroup => stmts.iter().collect(),
        // Stable-partition by group rank; order within a group stands.
        Style::Grouped => {
            let mut ordered: Vec<&UseStmt<'_>> = stmts.iter().collect();
            ordered.sort_by_key(|stmt| stmt.rank);
            ordered
        }
    };

    let mut out = String::new();
    for (index, stmt) in ordered.iter().enumerate() {
        if index > 0 {
            let blanks = match style {
                Style::SingleGroup => 0,
                Style::Grouped if ordered[index - 1].rank == stmt.rank => 0,
                Style::Grouped => blank_line_count,
            };
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
