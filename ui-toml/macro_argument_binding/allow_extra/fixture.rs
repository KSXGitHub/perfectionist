// Skipped: `my_macro!` is uncatalogued and would normally be flagged
// under the default `AllowAndDeny` mode, but the fixture's
// `dylint.toml` adds `my_macro` to `allow_extra`, opting the macro
// into the trusted set. Even a non-trivial argument is now accepted.

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
