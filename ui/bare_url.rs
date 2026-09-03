// Bad: bare URL in a plain line comment — see https://example.com for details.
// Good: <https://example.com> wrapped explicitly.

/* Bad: bare URL in a block comment — https://example.org. */
/* Good: <https://example.org>. */

/// Bad: bare URL in a doc comment — see https://example.com for details.
fn _doc_bad() {}

/** Bad: bare URL in a `/** */` doc-block comment — https://example.org. */
fn _doc_block_bad() {}

mod _inner_doc_block {
    /*! Bad: bare URL in an inner `/*! */` doc-block — https://example.com. */
}

/// Good: <https://example.com> wrapped explicitly.
fn _doc_good_wrapped() {}

/// Good: [the example site](https://example.com) as a labelled link.
fn _doc_good_labelled() {}

/// Inside a code span: `https://example.com` should not fire — the
/// markdown code-span detection suppresses it (the host is not in
/// the default `skip_hosts`, so a skip here proves the span logic).
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

/// Regression for the `at_line_start` fix: a fenced code block
/// immediately followed by a reference-link definition. Both must
/// be classified as skip regions, even though the first ends at a
/// line boundary that the second relies on.
/// ```
/// let url = "https://example.com";
/// ```
/// [home]: https://example.com
fn _doc_adjacent_block_constructs() {}

/// Default skip-host `localhost` should not fire: https://localhost here.
fn _doc_skip_host() {}

/// Skip-host comparison is case-insensitive (RFC 3986): https://LOCALHOST stays quiet.
fn _doc_skip_host_case_insensitive() {}

/// URL already inside an HTML attribute: <a href="https://example.com">click</a>.
fn _doc_html_attribute_skip() {}

// URL with trailing dot: https://example.org. — should be `MaybeIncorrect`.
fn _plain_trailing_dot() {}

// A finding anchors to the nearest documentable HIR node. Items and
// impl items are covered above; trait items and foreign items are
// separate arms of that walk.
trait _DocumentedTrait {
    /// Bad: bare URL on a trait item — https://example.com/trait.
    fn documented_method(&self);
}

extern "C" {
    /// Bad: bare URL on a foreign item — https://example.com/foreign.
    fn documented_foreign();
}

fn main() {
    let _ = "https://not-a-comment.example"; // string literal, ignored
}
