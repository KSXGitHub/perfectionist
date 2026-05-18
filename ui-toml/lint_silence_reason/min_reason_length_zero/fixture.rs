// `min_reason_length = 0` disables the length branch entirely;
// `reason` presence is still enforced.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

// Good — even a one-character `reason` is accepted.
#[allow(dead_code, reason = "x")]
fn one_char_reason() {}

// Bad — presence is still enforced.
#[allow(dead_code)]
fn no_reason() {}

fn main() {
    one_char_reason();
    no_reason();
}
