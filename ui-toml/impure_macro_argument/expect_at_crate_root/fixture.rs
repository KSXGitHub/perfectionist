// Crate-level `#[expect]` must mark the expectation fulfilled when
// a violation exists in the crate. Mirrors the equivalent fixture in
// `macro_trailing_comma`'s test suite.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
#![cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::impure_macro_argument,
        reason = "crate-root expect must fulfil"
    )
)]

fn main() {
    debug_assert!(value().is_some());
}

fn value() -> Option<u32> {
    None
}
