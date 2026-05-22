// Bad: bare email in a plain line comment — report to security@rust-lang.org.
// Good (angle): <security@rust-lang.org>.
// Good (mailto): mailto:security@rust-lang.org.

/* Bad: bare email in a block comment — security@rust-lang.org. */

/// Bad: bare email in a doc comment — write to security@rust-lang.org.
fn _doc_bad() {}

/** Bad: bare email in a `/** */` doc-block comment — security@rust-lang.org. */
fn _doc_block_bad() {}

/// Good: <security@rust-lang.org> wrapped explicitly.
fn _doc_wrapped() {}

/// Good: mailto:security@rust-lang.org prefixed.
fn _doc_mailto() {}

/// Inside a code span: `user@example.com` should not fire (and also the
/// default `skip_domains` would have caught it anyway).
fn _doc_code_span() {}

/// Default-skip domain: someone@example.com — no warning.
fn _doc_skip_domain() {}

/// A version-like token `crate@1.2.3` is not an email and must not fire.
fn _doc_not_email() {}

fn main() {
    // Email inside a comment, not in source: still flagged.
    let _ = "not_a_comment@example.com"; // skipped — example.com domain
}
