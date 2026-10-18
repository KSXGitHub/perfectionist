// A per-item `#[expect]` must both suppress the finding and be
// fulfilled by it, and a module-scope one must fulfil too. This
// fixture produces no diagnostics at all.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::em_dash_prose,
        reason = "quotes the dash on purpose"
    )
)]
/// Mentions the em dash — on purpose.
pub fn documented() {}

mod inner {
    #![cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::em_dash_prose,
            reason = "module-scope expect must fulfil too"
        )
    )]

    //! Inner module doc comment with an em dash — is suppressed here.
}

fn main() {}
