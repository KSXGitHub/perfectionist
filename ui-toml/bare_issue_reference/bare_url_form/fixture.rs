/// Bad: closes #55 — the `bare_url` form replaces the `#55` token
/// outright with the issue URL, dropping the token rather than
/// wrapping it in markdown.
fn _doc_bare_url_form() {}

// Bad: closes #56 — plain line comments are scanned too under
// `include_plain_comments = true`, and take their shape from
// `plain_comment_form`, which is `bare_url` by default.
fn _plain_bare_url_form() {}

fn main() {}
