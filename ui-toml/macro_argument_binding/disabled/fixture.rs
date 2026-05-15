// Skipped: `debug_assert_eq!` is on the built-in deny list and the
// first argument is a non-trivial method call, but the test driver
// supplies a `[perfectionist] disable = ["macro_argument_binding"]`
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
