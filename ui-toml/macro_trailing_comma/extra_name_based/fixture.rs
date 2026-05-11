// Bad: `my_macro!` is not on the built-in name-based list, but the
// fixture's `dylint.toml` adds it via `extra_name_based`, so the
// missing multi-line trailing comma must be flagged.

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
