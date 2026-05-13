// Bad: `my_macro!` is not on the built-in name-based list. The
// fixture's `dylint.toml` adds the bare segment `"my_macro"` to
// `name_based_extra`, so the missing multi-line trailing comma must
// be flagged. A `macro_rules!` is used here only because it lets the
// fixture be self-contained — bare-segment matching applies equally
// to third-party macros (`use somecrate::their_macro; their_macro!(...)`).

macro_rules! my_macro {
    ($($item:expr),* $(,)?) => {{ $(let _ = $item;)* 0 }};
}

fn main() {
    let _ = my_macro!(
        1,
        2,
        3
    );
}
