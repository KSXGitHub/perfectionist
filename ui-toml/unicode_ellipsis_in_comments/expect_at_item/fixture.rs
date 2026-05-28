// Regression for
// <https://github.com/KSXGitHub/perfectionist/issues/165>: a plain
// `//` comment is not an attribute, so it anchors at the deepest HIR
// node whose body span contains it — here the function body — and a
// `#[expect]` on the enclosing function both suppresses the finding
// and is fulfilled by it. The fixture produces no diagnostics; before
// the fix every finding resolved to the crate root.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::unicode_ellipsis_in_comments,
        reason = "the comment names the ellipsis glyph on purpose"
    )
)]
fn documented() {
    // A plain comment naming the ellipsis … on purpose.
    let _ = 1;
}

fn main() {
    documented();
}
