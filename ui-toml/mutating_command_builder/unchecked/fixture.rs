// edition:2024
//
// `command_extra_dependency = "unchecked"`. Nothing declares
// `command_extra`, so the diagnostic names a method this crate cannot yet
// call — which is the point of the value: a project deciding whether to
// take the dependency at all wants to see what it would buy.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::process::Command;

fn without_the_crate() {
    let mut command = Command::new("ls");
    command.arg("-l");
    command.env_clear();
}

// The remedy beside a rendered rename, which is the other half of the
// pair `ui/mutating_command_builder.rs` covers: the rename is a block
// with a span, printed below every span-less line, so the remedy rides
// in its message instead of arriving above the advice.
fn rename_beside_the_remedy() {
    Command::new("ls").arg("-a");
}

fn main() {}
