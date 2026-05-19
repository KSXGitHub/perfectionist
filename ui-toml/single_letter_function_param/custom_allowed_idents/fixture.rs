#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, reason = "ui fixture")]
#![warn(perfectionist::single_letter_function_param)]

// `extra_allowed_idents = ["x"]` adds to the built-in
// `["n", "f", "i", "j", "k"]` list, and `ignore_allowed_idents = ["n"]`
// drops the default back out. After the merge `n` is no longer
// allowed and `x` is.

// `n` is dropped from the allowlist by `ignore_allowed_idents`: fires.
fn count(n: usize) -> usize {
    n + 1
}

// `x` is added to the allowlist by `extra_allowed_idents`: quiet.
fn squared(x: i32) -> i32 {
    x * x
}

fn main() {}
