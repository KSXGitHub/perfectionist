// Bad: a plain line comment leaves an address bare: security@example.com.
// Good (angle): <security@example.com>.
// Good (mailto): mailto:security@example.com.

/* Bad: a block comment leaves an address bare: security@example.org. */

/// Bad: a doc comment leaves an address bare: security@example.com.
fn _doc_bad() {}

/** Bad: a `/** */` doc-block leaves an address bare: security@example.org. */
fn _doc_block_bad() {}

/// Good: <security@example.com> wrapped explicitly.
fn _doc_wrapped() {}

/// Good: mailto:security@example.com prefixed.
fn _doc_mailto() {}

/// Inside a code span: `user@example.com` should not fire, since
/// the markdown code-span detection suppresses it (the domain is not
/// in the default `skip_domains`, so a skip here proves the span
/// logic).
fn _doc_code_span() {}

/// A version-like token `crate@1.2.3` is not an email and must not fire.
fn _doc_not_email() {}

fn main() {
    // Email inside a string literal, not a comment: never scanned.
    let _ = "not_a_comment@example.com";
}
