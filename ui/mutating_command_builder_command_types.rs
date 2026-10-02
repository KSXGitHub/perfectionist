// aux-build:tokio_stub.rs
// aux-build:async_process_stub.rs
// aux-build:command_extra_1_5_0.rs
// edition:2024
//
// Every receiver `CommandExtra` covers, each flagged for one reason: the
// setter takes `&mut self`, so the chain cannot yield the command, and
// the trait has a by-value counterpart for that very type.
//
// `command-extra` implements the trait for `Box<Command>`
// unconditionally and, behind a feature apiece, for `tokio::process`'s
// and `async_process`'s own `Command` and each of those boxed. The rule
// asks the trait rather than naming a crate, so which receivers it
// reaches is the resolved release's decision, and a release implementing
// the trait for something new is followed with no change here.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate async_process;
extern crate command_extra;
extern crate tokio;

use command_extra::CommandExtra;

// Bad: std's own `Command`.
fn bare_command() {
    let mut command = std::process::Command::new("ls");
    command.arg("bare-command");
}

// Bad: a box, whose `arg` is still std's, reached through the box.
fn boxed_command() {
    let mut command = Box::new(std::process::Command::new("ls"));
    command.arg("boxed-command");
}

// Bad: tokio's own `Command`, whose `arg` is tokio's method rather than
// std's.
fn tokio_command() {
    let mut command = tokio::process::Command::new("ls");
    command.arg("tokio-command");
}

// Bad: tokio's command, boxed.
fn boxed_tokio_command() {
    let mut command = Box::new(tokio::process::Command::new("ls"));
    command.arg("boxed-tokio-command");
}

// Bad: async-process's own `Command`, whose `arg` is its own method.
fn async_command() {
    let mut command = async_process::Command::new("ls");
    command.arg("async-command");
}

// Bad: async-process's command, boxed.
fn boxed_async_command() {
    let mut command = Box::new(async_process::Command::new("ls"));
    command.arg("boxed-async-command");
}

// Not flagged: the counterpart called directly, so there is no std
// setter to report. An impl going missing makes this fixture `E0599`,
// which is what keeps the diagnostics above attributable to the trait.
fn the_counterparts_exist() {
    let _ = Box::new(std::process::Command::new("ls")).with_arg("a");
    let _ = tokio::process::Command::new("ls").with_arg("a");
    let _ = Box::new(tokio::process::Command::new("ls")).with_arg("a");
    let _ = async_process::Command::new("ls").with_arg("a");
    let _ = Box::new(async_process::Command::new("ls")).with_arg("a");
}

fn main() {}
