// With `disable = ["allow_attributes_without_reason"]` in the `[perfectionist]`
// table, the rule's pass is not installed and the missing `reason`
// is silent.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]

#[allow(dead_code)]
fn no_reason_but_rule_disabled() {}

fn main() {
    no_reason_but_rule_disabled();
}
