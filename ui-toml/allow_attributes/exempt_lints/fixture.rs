// `extra_exempt_lints = ["clippy::too_many_arguments"]` adds an
// exemption, and `ignore_exempt_lints = ["dead_code"]` drops a default
// one: the clippy lint becomes exempt while `dead_code` is rewritten.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

// Good — now exempt via `extra_exempt_lints`.
#[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
fn now_exempt() {}

// Bad — `dead_code` was dropped from the exempt set via
// `ignore_exempt_lints`, so the rewrite applies.
#[allow(dead_code, reason = "kept around for symmetry")]
fn dead_code_now_rewritten() {}

fn main() {
    now_exempt();
    dead_code_now_rewritten();
}
