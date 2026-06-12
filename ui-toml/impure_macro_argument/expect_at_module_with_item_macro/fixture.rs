// A module-scope `#[expect]` over a macro that expands at item
// position (e.g. produces a `pub const`) should fulfil — the late
// pass finds the deepest enclosing HIR node for each violation and
// emits the diagnostic there, so a per-module suppression behaves
// correctly regardless of whether the resulting item is at function
// scope or item scope. Regression test for
// <https://github.com/KSXGitHub/parallel-disk-usage/issues/419>.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

const fn produce_const(input: u32) -> u32 {
    input
}

#[cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::impure_macro_argument,
        reason = "module-scope expect must fulfil for item-position macros"
    )
)]
mod inner {
    use super::produce_const;

    macro_rules! define_const {
        ($name:ident, $value:expr) => {
            pub const $name: u32 = $value;
        };
    }

    define_const!(MY_CONST, produce_const(5));
}

fn main() {
    let _ = inner::MY_CONST;
}
