// `min_reason_length = 1` is the floor's lowest legal value
// (`0` is rejected at parse time by the `NonZeroUsize` field).
// At this setting every non-blank reason satisfies the length
// branch; presence is still enforced, and a blank literal is
// still treated as if the field were missing.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_reason_from_comment,
    reason = "fixture targets `lint_silence_reason`; the trailing/leading comments are documentation, not rationales to lift",
)]

// Good — even a one-character `reason` is accepted.
#[allow(dead_code, reason = "x")]
fn one_char_reason() {}

// Bad — presence is still enforced.
#[allow(dead_code)]
fn no_reason() {}

// Bad — empty literal counts as missing regardless of the
// length floor.
#[allow(dead_code, reason = "")]
fn empty_reason() {}

// Bad — whitespace-only literal also counts as missing,
// regardless of the length floor.
#[allow(dead_code, reason = "   ")]
fn whitespace_only_reason() {}

fn main() {
    one_char_reason();
    no_reason();
    empty_reason();
    whitespace_only_reason();
}
