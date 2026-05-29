// `exempt_lints = ["clippy::too_many_arguments"]` replaces the default
// exempt list outright: the configured lint becomes exempt, while a
// former default member like `dead_code` becomes rewriteable again.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

// Good — now exempt by configuration.
#[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
fn now_exempt() {}

// Bad — `dead_code` is no longer exempt once the default list is
// replaced, so the rewrite applies.
#[allow(dead_code, reason = "kept around for symmetry")]
fn dead_code_now_rewritten() {}

fn main() {
    now_exempt();
    dead_code_now_rewritten();
}
