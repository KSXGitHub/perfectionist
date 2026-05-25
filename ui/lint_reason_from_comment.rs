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

// Bad once: `cfg_attr` expanding to *two* synth lint-level attrs
// shares a single trailing comment. The rule lifts the comment
// onto the first synth attr only — emitting a suggestion for
// every synth would produce overlapping `delete_span`s on the
// shared comment that rustfix cannot apply.
#[cfg_attr(all(), allow(dead_code), allow(unused_variables))] // shared rationale
fn trailing_cfg_attr_multi_synth() {}

// Bad: nested `cfg_attr` keeps the *outermost* trace span as the
// comment-search anchor. If the rule overwrote the pending trace
// on each nested visit, the inner trace's narrower span would
// miss the trailing comment after the outer `]`.
#[cfg_attr(all(), cfg_attr(all(), allow(dead_code)))] // nested wrap
fn trailing_nested_cfg_attr() {}

// Bad once: cfg_attr's first synth already carries `reason`, so it
// doesn't emit; the pending trace must stay live so the *second*
// synth can still lift the trailing comment. (The bug version
// consumed the trace unconditionally after the first synth was
// visited, regardless of whether anything was emitted.)
#[cfg_attr(all(), allow(dead_code, reason = "explained"), allow(unused_variables))] // for second synth
fn cfg_attr_first_synth_has_reason() {}

// Good: a bare `//` trailing comment normalises to empty and is not
// lifted (no vacuous `reason = ""`).
#[allow(dead_code)] //
fn bare_trailing_comment_ignored() {}

// Good: an all-decoration trailing divider normalises to empty and
// is not lifted.
#[allow(dead_code)] //----------
fn divider_trailing_comment_ignored() {}

// Good: a comment on the line *above* the attribute is out of scope
// — only same-line trailing comments are lifted, never a comment
// that might instead be documenting the item below.
// documentation for the function, not the attribute
#[allow(dead_code)]
fn leading_comment_is_out_of_scope() {}

// Good: a comment on the line *after* the attribute is not trailing
// (it is not on the same line as the closing `]`).
#[allow(dead_code)]
// not a trailing comment
fn comment_on_next_line_is_out_of_scope() {}

// Good: attribute already carries `reason`; rule must not fire.
#[allow(dead_code, reason = "explicit reason")] // separate trailing note
fn already_has_reason() {}

fn main() {
    trailing_allow();
    trailing_warn();
    trailing_deny();
    trailing_forbid();
    trailing_decoration();
    trailing_multiline_no_comma();
    trailing_multiline_comma();
    trailing_escapes();
    trailing_cfg_attr();
    trailing_cfg_attr_multi_synth();
    trailing_nested_cfg_attr();
    cfg_attr_first_synth_has_reason();
    bare_trailing_comment_ignored();
    divider_trailing_comment_ignored();
    leading_comment_is_out_of_scope();
    comment_on_next_line_is_out_of_scope();
    already_has_reason();
}
