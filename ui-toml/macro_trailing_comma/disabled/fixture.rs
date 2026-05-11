// Skipped: `vec!` is on the built-in name-based list and the
// multi-line invocation lacks a trailing comma, but the fixture's
// `dylint.toml` sets `enabled = false`, so the rule must NOT fire.

fn main() {
    let _ = vec![
        1,
        2,
        3
    ];
}
