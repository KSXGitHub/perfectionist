// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #55 — the `bracketed_url` form replaces the `#55`
/// token with the issue URL wrapped in `<...>`, the autolink spelling
/// that keeps rustdoc from swallowing trailing punctuation.
fn _doc_bracketed_url_form() {}

// Bad: closes #56 — `plain_comment_form = "bracketed_url"` applies the
// same wrapping in plain line comments.
fn _plain_bracketed_url_form() {}

fn main() {}
