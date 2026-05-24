// `lift_trailing_comments = false`: the leading placement still
// fires, the trailing placement is silenced.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_silence_reason,
    reason = "fixture targets the comment-lift rule"
)]

// Good: a trailing-only comment is not lifted when trailing is
// disabled. The blank line below keeps this prose from being read as
// a leading comment for the attribute.

#[allow(dead_code)] // trailing silenced
fn trailing_silenced() {}

// leading fires
#[allow(dead_code)]
fn leading_fires() {}

fn main() {
    trailing_silenced();
    leading_fires();
}
