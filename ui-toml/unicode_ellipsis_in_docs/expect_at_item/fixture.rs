// A per-item `#[expect]` must both suppress the doc-comment finding
// and be fulfilled by it. The regression in
// <https://github.com/KSXGitHub/perfectionist/issues/165> anchored
// every comment-scanned finding at the crate root, so a per-item
// attribute was ignored: the finding still fired *and* the
// expectation was additionally reported as unfulfilled. With the
// finding now emitted at the comment's enclosing HIR node, this
// fixture produces no diagnostics at all.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::unicode_ellipsis_in_docs,
        reason = "documents the ellipsis glyph on purpose"
    )
)]
/// Mentions the ellipsis … on purpose.
pub fn documented() {}

mod inner {
    #![cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::unicode_ellipsis_in_docs,
            reason = "module-scope expect must fulfil too"
        )
    )]

    //! Inner module doc comment with an ellipsis … is suppressed here.
}

fn main() {}
