// Bad: bare URL in a plain line comment — see https://rust-lang.org for details.
// Good: <https://rust-lang.org> wrapped explicitly.

/* Bad: bare URL in a block comment — https://rust-lang.org. */
/* Good: <https://rust-lang.org>. */

/// Bad: bare URL in a doc comment — see https://rust-lang.org for details.
fn _doc_bad() {}

/** Bad: bare URL in a `/** */` doc-block comment — https://rust-lang.org. */
fn _doc_block_bad() {}

mod _inner_doc_block {
    /*! Bad: bare URL in an inner `/*! */` doc-block — https://rust-lang.org. */
}

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

/// Skip-host comparison is case-insensitive (RFC 3986): https://EXAMPLE.COM stays quiet.
fn _doc_skip_host_case_insensitive() {}

/// URL already inside an HTML attribute: <a href="https://rust-lang.org">click</a>.
fn _doc_html_attribute_skip() {}

// URL with trailing dot: https://rust-lang.org. — should be `MaybeIncorrect`.
fn _plain_trailing_dot() {}

fn main() {
    let _ = "https://not-a-comment.example"; // string literal, ignored
}
