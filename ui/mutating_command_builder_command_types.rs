// aux-build:tokio_stub.rs
// aux-build:command_extra_1_4_0_tokio.rs
// edition:2024
//
// The receivers this rule does *not* reach, pinned so that widening it
// is a measured change rather than a guess.
//
// `command-extra` 1.4.0 implements `CommandExtra` for `Box<Command>`
// unconditionally and, behind its `tokio_process` feature, for
// `tokio::process::Command` and `Box<tokio::process::Command>`. Each of
// those now has the by-value counterpart this rule exists to name, and
// each std-style setter below still takes `&mut self`, so the argument
// for flagging them is the same one the bare `Command` is flagged under.
//
// The rule asks whether the receiver's type *is* `std::process::Command`,
// so only the first function earns a diagnostic. The three that follow
// are the gap.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;
extern crate tokio;

use command_extra::CommandExtra;

// Bad: the receiver this rule has always reached.
fn bare_command() {
    let mut command = std::process::Command::new("ls");
    command.arg("bare-command");
}

// Not flagged, though `Box<Command>` has had `with_arg` since 1.4.0.
fn boxed_command() {
    let mut command = Box::new(std::process::Command::new("ls"));
    command.arg("boxed-command");
}

// Not flagged: `tokio::process::Command::arg` is tokio's method, not
// std's, so the setter table does not reach it.
fn tokio_command() {
    let mut command = tokio::process::Command::new("ls");
    command.arg("tokio-command");
}

// Not flagged, for both reasons at once.
fn boxed_tokio_command() {
    let mut command = Box::new(tokio::process::Command::new("ls"));
    command.arg("boxed-tokio-command");
}

fn main() {}
