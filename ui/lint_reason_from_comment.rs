#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
// Silence the sibling lint so this fixture only exercises
// `lint_reason_from_comment`.
#![allow(
    perfectionist::lint_silence_reason,
    reason = "fixture targets the comment-lift rule",
)]

// Bad: trailing comment on `#[allow]`.
#[allow(dead_code)] // matches upstream signature
fn trailing_allow() {}

// Bad: trailing comment on `#[expect]`. Not called so the
// expectation is fulfilled by `dead_code` and no extra warning
// appears in the stderr.
#[expect(dead_code)] // stub for upcoming change
fn trailing_expect() {}

// Bad: trailing comment on `#[warn]`.
#[warn(dead_code)] // downgrade during migration
fn trailing_warn() {}

// Bad: trailing comment on `#[deny]`.
#[deny(dead_code)] // hot path; tolerate nothing
fn trailing_deny() {}

// Bad: trailing comment on `#[forbid]`.
#[forbid(dead_code)] // module-wide policy
fn trailing_forbid() {}

// Bad: leading comment.
// pnpm's wire format is JSON-stringly typed
#[allow(dead_code)]
struct LeadingComment {
    field: u32,
}

// Bad: decoration prefix is stripped.
#[allow(dead_code)] //-- matches upstream signature
fn trailing_decoration() {}

// Bad: trailing comment with multi-line attribute, no trailing comma.
#[allow(
    dead_code
)] // multi-line layout
fn trailing_multiline_no_comma() {}

// Bad: trailing comment with multi-line attribute, trailing comma.
#[allow(
    dead_code,
)] // multi-line with comma
fn trailing_multiline_comma() {}

// Bad: comment containing characters that need escaping.
#[allow(dead_code)] // he said "yes" and used a \ backslash
fn trailing_escapes() {}

// Bad: `cfg_attr`-wrapped attribute with trailing comment.
#[cfg_attr(all(), allow(dead_code))] // cfg_attr wrap
fn trailing_cfg_attr() {}

// Good: attribute already carries `reason`; rule must not fire.
#[allow(dead_code, reason = "explicit reason")] // separate trailing note
fn already_has_reason() {}

// Good: doc comment is not in scope.
/// Outer doc comment on the next item, not a leading comment for
/// the attribute.
#[allow(dead_code, reason = "documented above")]
fn doc_comment_is_not_a_leading_comment() {}

// Good: comment on its own line after the attribute is not a
// trailing comment (the attribute has no comment on the same line
// as its `]`). The comment below documents the next item.
#[allow(dead_code, reason = "comment below is for the function")]
// not a trailing comment
fn comment_on_next_line() {}

// Good: blank line between comment and attribute disqualifies the
// leading placement.
// this comment is unrelated

#[allow(dead_code, reason = "blank line disqualifies leading lift")]
fn blank_line_between() {}

fn main() {
    trailing_allow();
    trailing_warn();
    trailing_deny();
    trailing_forbid();
    let _ = LeadingComment { field: 0 };
    trailing_decoration();
    trailing_multiline_no_comma();
    trailing_multiline_comma();
    trailing_escapes();
    trailing_cfg_attr();
    already_has_reason();
    doc_comment_is_not_a_leading_comment();
    comment_on_next_line();
    blank_line_between();
}
