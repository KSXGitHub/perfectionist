// aux-build:proc_macro_synth_binding.rs

// Regression test for
// <https://github.com/KSXGitHub/perfectionist/pull/166#issuecomment-4557181827>:
// a per-field / per-variant `#[expect(perfectionist::unicode_ellipsis_in_docs)]`
// must be honored even when the field's / variant's enclosing type
// carries a proc-macro `#[derive(...)]`. A custom derive routes the
// type's tokens through a proc-macro, giving the field/variant
// doc-comment spans a proc-macro expansion context; the comment-anchor
// finder must still resolve each finding to the documented field /
// variant so the `#[expect]` both suppresses it and is fulfilled.
//
// This fixture produces no diagnostics. Before the fix the findings
// resolved to a coarser node, so they fired anyway and the
// expectations were reported unfulfilled. Every Config in this crate
// derives `serde::Deserialize` and documents its fields with an
// intentional ellipsis, so this is the crate's own primary use case.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
#![allow(dead_code, reason = "ui fixture")]

extern crate proc_macro_synth_binding;

use proc_macro_synth_binding::SynthFieldRefBody;

#[derive(SynthFieldRefBody)]
struct DerivedStruct {
    #[cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::unicode_ellipsis_in_docs,
            reason = "documents the ellipsis glyph on purpose"
        )
    )]
    /// Field doc comment with an ellipsis … on purpose.
    field: u32,
}

#[derive(SynthFieldRefBody)]
enum DerivedEnum {
    #[cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::unicode_ellipsis_in_docs,
            reason = "documents the ellipsis glyph on purpose"
        )
    )]
    /// Variant doc comment with an ellipsis … on purpose.
    Variant,
}

fn main() {}
