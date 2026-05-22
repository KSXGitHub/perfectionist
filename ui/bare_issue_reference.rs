// Bare `#NNN` in a plain `//` comment is NOT flagged with default config
// (`include_plain_comments = false`): fixes #999, supersedes #1000.

/// Bad: closes #123 and supersedes #124.
fn _doc_bad() {}

/// Good (inline): [#123](https://github.com/owner/repo/issues/123).
fn _doc_inline_good() {}

/// Good (reference): see [#125].
///
/// [#125]: https://github.com/owner/repo/issues/125
fn _doc_reference_good() {}

/// Inside a code span: `#42` should not fire.
fn _doc_code_span() {}

/// URL fragments must not fire:
/// see https://example.com/issues/#999 and
/// https://example.com/path?ref=#1000 for more.
fn _doc_url_fragment() {}

fn main() {
    let _ = "#400"; // string literal, ignored
}
