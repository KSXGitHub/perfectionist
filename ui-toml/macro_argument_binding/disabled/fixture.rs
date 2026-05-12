// Skipped: `debug_assert_eq!` is on the built-in deny list and the
// first argument is a non-trivial method call, but the fixture's
// `dylint.toml` sets `enabled = false`, so the rule must NOT fire.

fn main() {
    let mut value: u32 = 0;
    debug_assert_eq!(replace(&mut value, 1), 0);
}

fn replace(slot: &mut u32, new: u32) -> u32 {
    let old = *slot;
    *slot = new;
    old
}
