// A per-item `#[expect]` must both suppress the finding and be
// fulfilled by it, and a module-scope one must fulfil too. This
// fixture produces no diagnostics at all.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

// Not flagged: the per-item `expect` suppresses the finding and is
// fulfilled by it.
#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::em_dash_prose,
        reason = "this item's documentation uses the dash on purpose"
    )
)]
/// Walks the tree — and returns the total size.
pub fn documented() {}

// Not flagged: a module-scope `expect` suppresses the finding and is
// fulfilled by it.
mod inner {
    #![cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::em_dash_prose,
            reason = "module-scope expect must fulfil too"
        )
    )]

    //! Rebuilds the index — the previous one was stale.
}

fn main() {}
