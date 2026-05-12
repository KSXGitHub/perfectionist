// Skipped: `debug_assert_eq!` is on the built-in deny list and the
// first argument is a non-trivial method call, but the fixture's
// `dylint.toml` adds `debug_assert_eq` to `ignore`, which always
// wins over the deny / allow lists. The rule emits no diagnostic.

fn main() {
    let mut value: u32 = 0;
    debug_assert_eq!(replace(&mut value, 1), 0);
}

fn replace(slot: &mut u32, new: u32) -> u32 {
    let old = *slot;
    *slot = new;
    old
}
