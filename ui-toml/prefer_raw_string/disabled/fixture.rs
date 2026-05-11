// Skipped: the literal `"foo\\bar"` would normally fire (one
// eliminable `\\` escape), but the fixture's `dylint.toml` sets
// `enabled = false`, so the rule must NOT fire.

fn main() {
    let _ = "foo\\bar";
}
