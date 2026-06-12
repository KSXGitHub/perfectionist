// Skipped: `debug_assert_eq!` is in the built-in deny set and the
// first argument is an impure method call, but the fixture's
// `dylint.toml` adds `debug_assert_eq` to `ignore`, which always
// wins over the deny / allow sets. The rule emits no diagnostic.

fn main() {
    let mut value: u32 = 0;
    debug_assert_eq!(replace(&mut value, 1), 0);
}

fn replace(slot: &mut u32, new: u32) -> u32 {
    let old = *slot;
    *slot = new;
    old
}
