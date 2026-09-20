// edition:2024
//
// `require_command_extra_dependency = false`. No `command_extra` is
// declared, so the diagnostic names a method this crate cannot yet call —
// which is the point of turning the gate off: a workspace that adds the
// dependency per-crate wants to be told where it is still missing.

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
