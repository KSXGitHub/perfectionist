#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, unused_variables, reason = "ui fixture")]
#![warn(perfectionist::single_letter_let_binding)]

// `extra_allowed_idents = ["x"]` adds to the built-in `["n"]` list,
// and `extra_denied_idents = ["n"]` drops the default back out.
// After the merge `n` is no longer allowed and `x` is.

fn run() {
    // `n` is dropped from the allowlist by `extra_denied_idents`: fires.
    let n = 5_u32;
    // `x` is added to the allowlist by `extra_allowed_idents`: quiet.
    let x = 10_u32;
}

fn main() {
    run();
}
