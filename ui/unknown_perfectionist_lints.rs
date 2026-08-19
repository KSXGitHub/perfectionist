#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    perfectionist::allow_attributes_without_reason,
    perfectionist::lint_attribute_trailing_comment,
    perfectionist::allow_attributes,
    reason = "ui fixture",
)]

// Bad: typo of `unicode_ellipsis_in_comments` (missing trailing `s`).
#[allow(perfectionist::unicode_ellipsis_in_comment)]
fn typo_close() {}

// Bad: typo of `unknown_perfectionist_lints` (missing trailing `s`),
// under `deny`.
#[deny(perfectionist::unknown_perfectionist_lint)]
fn typo_close_two() {}

// Bad: word fragment dropped — three edits from
// `unicode_ellipsis_in_comments`, the threshold's limit.
#[allow(perfectionist::unicode_ellipsis_comments)]
fn typo_dropped_fragment() {}

// Bad: depth mismatch — two trailing segments. The joined form
// (`unknown_perfectionist_lints`) is itself a registered name, so a
// "did you mean" hint applies.
#[allow(perfectionist::unknown::perfectionist_lints)]
mod depth_mismatch {}

// Bad: tool prefix with no lint name.
#[allow(perfectionist)]
fn no_target() {}

// Bad: nothing close enough for the suggestion threshold.
#[warn(perfectionist::nothing_like_this_anywhere)]
fn no_suggestion() {}

// Good: registered name.
#[allow(perfectionist::unicode_ellipsis_in_comments)]
fn known_one() {}

// Good: this rule itself is registered, so its own suppression site
// does not warn.
#[allow(perfectionist::unknown_perfectionist_lints)]
fn known_two() {}

// Good: different tool namespace; out of scope.
#[allow(clippy::needless_return)]
fn other_tool() {}

// Good: bare lint name; out of scope (rustc's `unknown_lints` owns it).
#[allow(path_qualification_mismatch)]
fn bare() {}

fn main() {}
