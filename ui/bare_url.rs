// Bad: bare URL in a plain line comment — see https://rust-lang.org for details.
// Good: <https://rust-lang.org> wrapped explicitly.

/* Bad: bare URL in a block comment — https://rust-lang.org. */
/* Good: <https://rust-lang.org>. */

/// Bad: bare URL in a doc comment — see https://rust-lang.org for details.
fn _doc_bad() {}

/// Good: <https://rust-lang.org> wrapped explicitly.
fn _doc_good_wrapped() {}

/// Good: [Rust homepage](https://rust-lang.org) as a labelled link.
fn _doc_good_labelled() {}

/// Inside a code span: `https://example.com` should not fire.
fn _doc_code_span_skip() {}

/// Inside a code block, also fine:
/// ```
/// let url = "https://example.com";
/// ```
fn _doc_code_block_skip() {}

/// In a reference-link definition is fine too:
///
/// [home]: https://example.com
fn _doc_ref_def_skip() {}

/// Default skip-host `example.com` should not fire: https://example.com here.
fn _doc_skip_host() {}

// URL with trailing dot: https://rust-lang.org. — should be `MaybeIncorrect`.
fn _plain_trailing_dot() {}

fn main() {
    let _ = "https://not-a-comment.example"; // string literal, ignored
}
