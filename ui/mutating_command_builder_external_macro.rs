// aux-build:command_extra.rs
// aux-build:external_command_macro.rs
// edition:2024
//
// Regression test: a `Command` setter written at the call site of
// another crate's `macro_rules!`, which wraps it in an item of the
// macro's own. The setter's tokens are the caller's, so the guard on
// the method segment's span does not see the expansion, and the source
// under that span is the author's, so reading it does not either. What
// keeps this quiet is the check on the enclosing item's `def_span`.
//
// `command_extra` is pulled in so the dependency gate passes and that
// check is the only thing left that can keep the fixture silent.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]
#![expect(
    perfectionist::impure_macro_arguments,
    reason = "the setter has to be written at the call site for its span to be the caller's"
)]

extern crate command_extra;
extern crate external_command_macro;

external_command_macro::wrap_in_a_function!(std::process::Command::new("ls").arg("-l"));

fn main() {}
