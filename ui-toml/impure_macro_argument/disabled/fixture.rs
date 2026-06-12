// Skipped: `debug_assert_eq!` is in the built-in deny set and the
// first argument is an impure method call, but the test driver
// supplies a `[perfectionist] disable = ["impure_macro_argument"]`
// global config, so the rule's pass is never installed and no
// diagnostic fires.

fn main() {
    let mut value: u32 = 0;
    debug_assert_eq!(replace(&mut value, 1), 0);
}

fn replace(slot: &mut u32, new: u32) -> u32 {
    let old = *slot;
    *slot = new;
    old
}
