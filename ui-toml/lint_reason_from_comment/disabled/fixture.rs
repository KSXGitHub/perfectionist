// With `disable = ["lint_reason_from_comment"]` in the
// `[perfectionist]` table, the rule's pass is not installed and the
// adjacent comments are left alone.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_silence_reason,
    reason = "fixture targets the comment-lift rule"
)]

// Trailing comment, rule disabled: no diagnostic.
#[allow(dead_code)] // would normally lift
fn trailing_but_rule_disabled() {}

fn main() {
    trailing_but_rule_disabled();
}
