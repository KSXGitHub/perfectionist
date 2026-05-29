#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(
    perfectionist::lint_reason_from_comment,
    reason = "the leading comments document each case; they are not rationales to lift",
)]

// Bad: a built-in lint that is not exempt — simple `allow` -> `expect` swap.
#[allow(non_snake_case, reason = "external API uses camelCase")]
fn simple_builtin() {}

// Bad: a `clippy::*` lint is always rewriteable.
#[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
fn simple_clippy() {}

// Bad: a `rustdoc::*` lint is always rewriteable.
#[allow(rustdoc::broken_intra_doc_links, reason = "links resolve downstream")]
fn simple_rustdoc() {}

// Bad: a `perfectionist::*` tool-namespace lint (rewriteable by default).
#[allow(perfectionist::single_letter_function_param, reason = "math convention")]
fn simple_perfectionist(x: i32) -> i32 {
    x
}

// Bad: multi-line single-lint attribute, still a simple swap.
#[allow(
    clippy::type_complexity,
    reason = "the type mirrors the upstream alias",
)]
fn multiline_simple() {}

// Bad: `cfg_attr`-wrapped — swap only the inner `allow` identifier.
#[cfg_attr(all(), allow(clippy::too_many_arguments, reason = "cfg-gated"))]
fn cfg_attr_simple() {}

// Bad: nested `cfg_attr` — the walker reaches the inner `allow`.
#[cfg_attr(all(), cfg_attr(all(), allow(clippy::type_complexity, reason = "nested")))]
fn cfg_attr_nested() {}

// Bad: mixed exempt + rewriteable — split into `allow` + `expect`.
#[allow(dead_code, clippy::too_many_arguments, reason = "scaffolding")]
fn split_mixed() {}

// Bad: `cfg_attr`-wrapped split — the inner meta item splits in place.
#[cfg_attr(all(), allow(dead_code, clippy::type_complexity, reason = "split under cfg"))]
fn split_cfg_attr() {}

// Good: every named lint is exempt — `allow` stays.
#[allow(dead_code, reason = "used only in integration tests")]
struct ExemptOnly;

// Good: an unknown bare name might belong to a conditional plugin.
#[allow(this_is_not_a_real_lint, reason = "left alone")]
fn unknown_bare() {}

// Good: a lint *group* fires only if some member fires — left alone.
#[allow(unused, reason = "group, not an individual lint")]
fn group_left_alone() {}

// Good: clippy lint *groups* are also non-deterministic — left alone,
// even though individual `clippy::*` lints are rewriteable.
#[allow(clippy::pedantic, reason = "group, not an individual lint")]
fn clippy_group_left_alone() {}

// Good: `#[warn]` / `#[deny]` / `#[forbid]` are out of scope.
#[warn(non_snake_case)]
fn warn_out_of_scope() {}

// Good: outer `#[allow]` on a module item is left alone by default.
#[allow(non_snake_case, reason = "module-wide")]
mod outer_module {
    pub fn Item() {}
}

fn main() {
    // Good: an inner attribute (`#![...]`) is left alone by default.
    #![allow(non_snake_case, reason = "inner attr")]

    outer_module::Item();
    let _ = simple_perfectionist(1);
}
