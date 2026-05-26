// Regression for
// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
// `#[expect]` on the documented item both suppresses the bare-URL
// finding in its doc comment and is fulfilled by it. The fixture
// produces no diagnostics; before the fix the finding resolved to the
// crate root, so it fired anyway and the expectation went unfulfilled.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::bare_url,
        reason = "the URL is shown verbatim on purpose"
    )
)]
/// See https://example.com/path for details.
pub fn documented() {}

fn main() {}
