// `disable = ["prefer_expect_over_allow"]` in the `[perfectionist]`
// table skips this rule's pass entirely; the rewriteable `#[allow]`
// below produces no diagnostic.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

#[allow(clippy::too_many_arguments, reason = "rule disabled")]
fn would_fire_if_enabled() {}

fn main() {
    would_fire_if_enabled();
}
