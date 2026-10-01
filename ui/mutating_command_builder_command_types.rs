// aux-build:tokio_stub.rs
// aux-build:async_process_stub.rs
// aux-build:command_extra_1_5_0.rs
// edition:2024
//
// The receivers this rule does *not* reach, pinned so that widening it
// is a measured change rather than a guess.
//
// `command-extra` implements `CommandExtra` for `Box<Command>`
// unconditionally and, behind a feature apiece, for `tokio::process`'s
// and `async_process`'s own `Command` and each of those boxed. Every one
// of them has the by-value counterpart this rule exists to name, and
// every std-style setter below still takes `&mut self`, so the argument
// for flagging them is the same one the bare `Command` is flagged under.
//
// The rule asks whether the receiver's type *is* `std::process::Command`,
// so only the first function earns a diagnostic. The rest are the gap,
// and the list is every receiver the trait covers, so a widening that
// reaches some of them and not others shows up here.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate async_process;
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

// Not flagged: `async_process::Command::arg` is async-process's method,
// not std's, for the same reason tokio's is not.
fn async_command() {
    let mut command = async_process::Command::new("ls");
    command.arg("async-command");
}

// Not flagged, for both reasons at once.
fn boxed_async_command() {
    let mut command = Box::new(async_process::Command::new("ls"));
    command.arg("boxed-async-command");
}

// Not flagged, and here to keep the premise above honest: every receiver
// in this file has the by-value counterpart the diagnostic would name.
// `with_arg` is that counterpart, so an impl going missing makes this
// fixture `E0599` instead of leaving the silences above looking the same
// either way.
fn the_counterparts_exist() {
    let _ = Box::new(std::process::Command::new("ls")).with_arg("a");
    let _ = tokio::process::Command::new("ls").with_arg("a");
    let _ = Box::new(tokio::process::Command::new("ls")).with_arg("a");
    let _ = async_process::Command::new("ls").with_arg("a");
    let _ = Box::new(async_process::Command::new("ls")).with_arg("a");
}

fn main() {}
