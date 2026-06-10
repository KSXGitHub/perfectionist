#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_reason_from_comment,
    reason = "fixture targets `lint_silence_reason`; the trailing/leading comments are documentation, not rationales to lift",
)]

// Bad: `#[allow]` with no `reason`.
#[allow(dead_code)]
fn missing_reason() {}

// Bad: `#[expect]` with no `reason`. The function is not called so
// the underlying `dead_code` actually fires, fulfilling the
// expectation and leaving only this rule's diagnostic.
#[expect(dead_code)]
fn missing_reason_expect() {}

// Bad: multiple lints, none exempt, no `reason`.
#[allow(dead_code, unused_variables)]
fn missing_reason_multi() {}

// Bad: trailing comma, no `reason`.
#[allow(dead_code,)]
fn missing_reason_trailing_comma() {}

// Bad: multi-line, no `reason`.
#[allow(
    dead_code,
)]
fn missing_reason_multiline() {}

// Bad: `cfg_attr`-wrapped `#[expect]` with no `reason`. The
// expectation is fulfilled by `dead_code`.
#[cfg_attr(all(), expect(dead_code))]
fn missing_reason_cfg_attr() {}

// Bad: nested `cfg_attr` — the rule walks through both layers and
// flags the inner `allow`.
#[cfg_attr(all(), cfg_attr(all(), allow(dead_code)))]
fn missing_reason_nested_cfg_attr() {}

// Bad: `reason` is shorter than the default minimum of 3.
#[allow(dead_code, reason = "x")]
fn reason_too_short() {}

// Bad: `reason` length counted in characters, not bytes. "ré" is
// two chars (three bytes), still below the default floor of 3.
#[allow(dead_code, reason = "ré")]
fn reason_too_short_multibyte() {}

// Good: three multi-byte characters meet the floor.
#[allow(dead_code, reason = "résu")]
fn good_multibyte() {}

// Bad: empty `reason` is treated as if the field were absent.
// The autofix itself emits `reason = ""` as a placeholder, so an
// author who applies the suggestion and re-runs the linter sees
// this diagnostic until the placeholder is filled in.
#[allow(dead_code, reason = "")]
fn reason_empty() {}

// Bad: whitespace-only `reason` is treated the same as an empty
// literal — the literal is long enough for the default length
// floor but carries no rationale.
#[allow(dead_code, reason = "   ")]
fn reason_whitespace_only() {}

// Good: empty argument list — silences no lint, so the rule
// doesn't fire even without a `reason`.
#[allow()]
fn allow_no_lints() {}

// Bad: multi-line `cfg_attr` — the autofix should target the
// inner `allow` argument list, not the outer `cfg_attr` one.
#[cfg_attr(
    all(),
    allow(dead_code),
)]
fn missing_reason_multiline_cfg_attr() {}

// Bad: inner-attribute form (`#![...]`). The autofix's snippet
// starts with `#![` rather than `#[`; the scanner ignores the
// prefix and still finds the inner argument list.
mod inner_attribute_bare {
    #![allow(dead_code)]

    pub(super) fn _used_so_dead_code_fires() {}
}

// Good: `reason` of length 3.
#[allow(dead_code, reason = "ok!")]
fn good_min_length() {}

// Good: full rationale.
#[allow(dead_code, reason = "exercised by the integration tests")]
fn good_full_reason() {}

// Good: `#[expect]` with a `reason`. Uncalled so `dead_code` fires
// and the expectation fulfills.
#[expect(dead_code, reason = "stub for upcoming change")]
fn good_expect_reason() {}

// Good: `#[warn]` is out of scope for this rule.
#[warn(dead_code)]
fn warn_out_of_scope() {}

// Good: `#[deny]` is out of scope.
#[deny(dead_code)]
fn deny_out_of_scope() {}

// Good: `#[forbid]` is out of scope.
#[forbid(dead_code)]
fn forbid_out_of_scope() {}

// Call the `warn`/`deny`/`forbid`-attributed functions so their
// own `dead_code` doesn't fire; leave every `#[expect]` function
// uncalled so each expectation fulfills.
fn main() {
    warn_out_of_scope();
    deny_out_of_scope();
    forbid_out_of_scope();
}
