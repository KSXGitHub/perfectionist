// Bad: bare email in a plain line comment — report to security@example.com.
// Good (angle): <security@example.com>.
// Good (mailto): mailto:security@example.com.

/* Bad: bare email in a block comment — security@example.org. */

/// Bad: bare email in a doc comment — write to security@example.com.
fn _doc_bad() {}

/** Bad: bare email in a `/** */` doc-block comment — security@example.org. */
fn _doc_block_bad() {}

/// Good: <security@example.com> wrapped explicitly.
fn _doc_wrapped() {}

/// Good: mailto:security@example.com prefixed.
fn _doc_mailto() {}

/// Inside a code span: `user@example.com` should not fire — the
/// markdown code-span detection suppresses it (the domain is not
/// in the default `skip_domains`, so a skip here proves the span
/// logic).
fn _doc_code_span() {}

/// A version-like token `crate@1.2.3` is not an email and must not fire.
fn _doc_not_email() {}

fn main() {
    // Email inside a string literal, not a comment: never scanned.
    let _ = "not_a_comment@example.com";
}
