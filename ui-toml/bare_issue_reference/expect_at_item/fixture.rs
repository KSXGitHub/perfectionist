// Regression for
// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
// `#[expect]` on the documented item both suppresses the bare `#NNN`
// finding in its doc comment and is fulfilled by it. The fixture
// produces no diagnostics; before the fix the finding resolved to the
// crate root, so it fired anyway and the expectation went unfulfilled.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::bare_issue_reference,
        reason = "the reference is written bare on purpose"
    )
)]
/// Closes #123 and supersedes #124.
pub fn documented() {}

fn main() {}
