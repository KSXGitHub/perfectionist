// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Ellipsis inside an inline code span `let s = "…";` is flagged here.
fn _code_span() {}

/// Ellipsis in a fenced code block stays skipped:
///
/// ```
/// let s = "loading…";
/// ```
fn _fenced_block() {}

/// Prose ellipsis… is flagged regardless.
fn _prose() {}

fn main() {}
