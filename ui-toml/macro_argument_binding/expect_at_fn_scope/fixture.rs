// A function-scope `#[expect]` should fulfil — the late pass finds
// the deepest enclosing HIR node for each violation and emits the
// diagnostic there, so a per-function suppression behaves correctly.
// The crate-level case is exercised by the `expect_at_crate_root`
// fixture.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::macro_argument_binding,
        reason = "function-scope expect must still fulfil"
    )
)]
fn suppressed() {
    debug_assert!(value().is_some());
}

fn main() {
    suppressed();
}

fn value() -> Option<u32> {
    None
}
