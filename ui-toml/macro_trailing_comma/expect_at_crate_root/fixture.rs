// Regression for issue #409: a crate-level `#[expect]` of this lint
// must fulfil rustc's expectation when violations exist anywhere in
// the crate. The fixture wraps the suppression in `cfg_attr` — the
// shape downstream crates use to avoid sprinkling `dylint`-only
// attributes through the source — and verifies that the rule emits
// its diagnostic for rustc's `#[expect]` machinery to consume.

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
