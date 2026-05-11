// Regression for
// <https://github.com/KSXGitHub/parallel-disk-usage/issues/409>: a
// `cfg_attr`-wrapped crate-level `#[expect]` must fulfil rustc's
// expectation when violations exist in the crate.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
#![cfg_attr(
    dylint_lib = "perfectionist",
    expect(
        perfectionist::macro_trailing_comma,
        reason = "regression test for issue #409"
    )
)]

fn main() {
    let _ = vec![
        1,
        2,
        3
    ];
}
