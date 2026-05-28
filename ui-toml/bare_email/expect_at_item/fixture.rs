// Regression for
// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
// `#[expect]` on the documented item both suppresses the bare-email
// finding in its doc comment and is fulfilled by it. The fixture
// produces no diagnostics; before the fix the finding resolved to the
// crate root, so it fired anyway and the expectation went unfulfilled.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::bare_email,
        reason = "the address is shown verbatim on purpose"
    )
)]
/// Report security issues to security@example.com.
pub fn documented() {}

fn main() {}
