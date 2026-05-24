// `lift_trailing_comments = false` and `lift_leading_comments =
// false`: the pass installs but the early `check_attribute` guard
// returns before any scan, so nothing is flagged.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_silence_reason,
    reason = "fixture targets the comment-lift rule"
)]

// Good: trailing comment is not lifted.
#[allow(dead_code)] // trailing silenced
fn trailing_silenced() {}

// Good: leading comment is not lifted.
// leading silenced
#[allow(dead_code)]
fn leading_silenced() {}

fn main() {
    trailing_silenced();
    leading_silenced();
}
