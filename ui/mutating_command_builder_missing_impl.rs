// aux-build:tokio_stub.rs
// aux-build:command_extra.rs
// edition:2024
//
// A release that implements `CommandExtra` for the bare `Command` and
// nothing else. Which receivers the rule reaches is that release's
// decision, so the ones it does not bless are left alone: there is no
// counterpart to name for them, and the setter call compiles either way.
//
// The bare `Command` fold is here to keep the silence attributable to
// the missing impls rather than to the fixture.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;
extern crate tokio;

use command_extra::CommandExtra;

// Bad: the receiver this release does bless.
fn bare_command() {
    let mut command = std::process::Command::new("ls");
    command.arg("bare-command");
}

// Not flagged: the setter is std's own, so what leaves this alone is
// the receiver carrying no counterpart.
fn boxed_command() {
    let mut command = Box::new(std::process::Command::new("ls"));
    command.arg("boxed-command");
}

// Not flagged: tokio's `arg` is tokio's own method, so the receiver and
// the setter are both unblessed here.
fn tokio_command() {
    let mut command = tokio::process::Command::new("ls");
    command.arg("tokio-command");
}

fn main() {}
