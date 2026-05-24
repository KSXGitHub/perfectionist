// `lift_leading_comments = false`: the trailing placement still
// fires, the leading placement is silenced.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_silence_reason,
    reason = "fixture targets the comment-lift rule"
)]

// Bad: trailing comment is still lifted.
#[allow(dead_code)] // trailing fires
fn trailing_fires() {}

// Good: leading comment is not lifted when leading is disabled.
// leading does not fire
#[allow(dead_code)]
fn leading_silenced() {}

fn main() {
    trailing_fires();
    leading_silenced();
}
