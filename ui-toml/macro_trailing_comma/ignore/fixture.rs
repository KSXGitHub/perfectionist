// Skipped: `vec!` is on the built-in name-based list, but the
// fixture's `dylint.toml` lists it under `ignore`, so the multi-line
// invocation must NOT be flagged.

fn main() {
    let _ = vec![
        1,
        2,
        3
    ];
}
