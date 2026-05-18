// `min_reason_length = 0` disables the length branch entirely;
// `reason` presence is still enforced, and an empty literal is
// treated as if the field were missing regardless of the length
// floor.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

// Good — even a one-character `reason` is accepted.
#[allow(dead_code, reason = "x")]
fn one_char_reason() {}

// Bad — presence is still enforced.
#[allow(dead_code)]
fn no_reason() {}

// Bad — empty literal counts as missing even with the length
// floor disabled.
#[allow(dead_code, reason = "")]
fn empty_reason() {}

fn main() {
    one_char_reason();
    no_reason();
    empty_reason();
}
