// Skipped: `my_macro!` appears in both `name_based_extra` and
// `ignore`. Per the rule spec, `ignore` is checked first, so the
// missing multi-line trailing comma must NOT be flagged.

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
