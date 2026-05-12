// Skipped: `my_macro!` is added to both `deny_extra` and `ignore`.
// `ignore` is checked first and always wins, so the rule emits no
// diagnostic even though the deny-list entry would otherwise fire on
// the non-trivial argument.

macro_rules! my_macro {
    ($item:expr) => {{
        let _ = $item;
        0
    }};
}

fn main() {
    let _ = my_macro!(value());
}

fn value() -> u32 {
    0
}
