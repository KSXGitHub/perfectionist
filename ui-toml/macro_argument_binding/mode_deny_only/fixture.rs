// `mode = "deny_only"` flags only invocations of the curated deny
// list (`debug_assert*`). Every other macro is silently accepted —
// the uncatalogued `my_macro!` here gets a free pass even though its
// argument is an impure function call.

macro_rules! my_macro {
    ($item:expr) => {{
        let _ = $item;
        0
    }};
}

fn main() {
    let _ = my_macro!(value());
    debug_assert!(value().is_some());
}

fn value() -> Option<u32> {
    None
}
