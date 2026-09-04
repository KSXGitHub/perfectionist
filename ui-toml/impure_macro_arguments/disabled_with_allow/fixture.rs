#![feature(register_tool)]
#![register_tool(perfectionist)]

// Skipped, and the suppression still resolves: the test driver
// supplies a `[perfectionist] disable = ["impure_macro_arguments"]`
// global config, so the rule's pass is never installed, while its
// lint declaration registers either way. Nothing is reported — not
// the rule, and not rustc's `unknown_lints` for the attribute below.

#[allow(
    perfectionist::impure_macro_arguments,
    reason = "the crate turns this rule off globally",
)]
fn main() {
    let mut value: u32 = 0;
    debug_assert_eq!(replace(&mut value, 1), 0);
}

fn replace(slot: &mut u32, new: u32) -> u32 {
    let old = *slot;
    *slot = new;
    old
}
