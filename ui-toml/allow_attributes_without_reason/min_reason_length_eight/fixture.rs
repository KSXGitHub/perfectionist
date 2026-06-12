// `min_reason_length = 8` raises the floor; a five-character
// `reason` is now too short.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_attribute_trailing_comment,
    reason = "fixture targets `allow_attributes_without_reason`; the trailing/leading comments are documentation, not rationales to lift",
)]

// Bad — five characters, below the new floor of 8.
#[allow(dead_code, reason = "short")]
fn five_chars() {}

// Good — exactly eight characters meets the floor.
#[allow(dead_code, reason = "eight or")]
fn eight_chars() {}

fn main() {
    five_chars();
    eight_chars();
}
