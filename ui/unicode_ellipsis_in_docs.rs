//! Crate-level inner doc comment with ellipsis… is flagged.

/// Outer doc comment with ellipsis… is flagged.
fn _flagged_outer() {}

/// Good doc comment with three dots... is not flagged.
fn _good_outer() {}

/// Ellipsis inside an inline code span `let s = "…";` is skipped by default.
fn _code_span() {}

/// Example with a fenced code block:
///
/// ```
/// let s = "loading…";
/// ```
fn _fenced_block() {}

/// Example with an indented code block:
///
///     let s = "loading…";
fn _indented_block() {}

/// Multi-line doc comment, first line…
/// and a second line with ellipsis… too.
fn _multiline() {}

/** Block doc comment with ellipsis… is flagged. */
fn _block_doc() {}

mod inner {
    //! Inner doc comment with ellipsis… is flagged.
}

fn main() {
    // Regular comments are the sibling comment rule's job, not this one.
    let _ = "string literal with …"; // string literals are ignored
}
