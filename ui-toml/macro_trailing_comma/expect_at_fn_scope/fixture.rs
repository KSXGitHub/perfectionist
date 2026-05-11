// A function-scope `#[expect]` should also fulfil — the late pass
// finds the deepest enclosing HIR node for each violation and emits
// the diagnostic there, so a per-function suppression behaves
// correctly. (The crate-level case is exercised by the
// `expect_at_crate_root` fixture; this one guards against a
// regression where emitting at the crate root would miss
// function-level attributes.)

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::macro_trailing_comma,
        reason = "function-scope expect must still fulfil"
    )
)]
fn suppressed() {
    let _ = vec![
        1,
        2,
        3
    ];
}

fn main() {
    suppressed();
}
