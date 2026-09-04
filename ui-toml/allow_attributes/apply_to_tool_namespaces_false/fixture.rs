// `apply_to_tool_namespaces = false`: `clippy::*`, `rustdoc::*`, and
// built-in lints are still rewritten, but other tool namespaces such
// as `perfectionist::*` are left alone.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

// Bad — `clippy::*` is always rewriteable.
#[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
fn clippy_still_rewritten() {}

// Bad — `rustdoc::*` is always rewriteable.
#[allow(rustdoc::broken_intra_doc_links, reason = "links resolve downstream")]
fn rustdoc_still_rewritten() {}

// Bad — a built-in lint is always rewriteable.
#[allow(non_snake_case, reason = "external API uses camelCase")]
fn builtin_still_rewritten() {}

// Good — `perfectionist::*` is left alone when tool namespaces are off.
#[allow(perfectionist::single_letter_function_param, reason = "math convention")]
fn perfectionist_left_alone(x: i32) -> i32 {
    x
}

fn main() {
    clippy_still_rewritten();
    rustdoc_still_rewritten();
    builtin_still_rewritten();
    let _ = perfectionist_left_alone(1);
}
