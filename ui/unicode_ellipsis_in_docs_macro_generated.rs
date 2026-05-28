// Regression test for
// <https://github.com/KSXGitHub/perfectionist/pull/166#issuecomment-4562112957>:
// a per-site `#[expect]` forwarded through a `macro_rules!` onto a
// generated item (the shape `declare_tool_lint!` uses for every lint in
// this crate) must still resolve. The forwarded `#[doc]` lands on the
// generated item with a macro-expansion-context span that does not
// byte-match the root-context comment the walker scans, so the
// comment-anchor finder must resolve macro hygiene before comparing
// spans — otherwise the finding falls back to crate-root resolution and
// the item-level `#[expect]` neither suppresses nor is fulfilled.
//
// This fixture produces no diagnostics. The forwarded `#[expect]` is
// captured by `$(#[$attr:meta])*` and lands on the generated item
// alongside the `#[doc]`, so it resolves on the same node the finding
// anchors to.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
#![allow(dead_code, reason = "ui fixture")]

macro_rules! declare_with_docs {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        pub struct $name;
    };
}

declare_with_docs! {
    #[cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::unicode_ellipsis_in_docs,
            reason = "documents the ellipsis glyph on purpose"
        )
    )]
    /// Generated item doc comment with an ellipsis … on purpose.
    Generated
}

fn main() {}
