// aux-build:tokio_stub.rs
// aux-build:async_process_stub.rs
// aux-build:command_extra_1_4_0_tokio.rs
// edition:2024
//
// The same release with its `tokio` feature on, where the gated impls
// are present and the `async_process` ones do not exist yet. One build
// reaching some receivers and not others is what pins that the rule
// follows the impls rather than a list of crates.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate async_process;
extern crate command_extra;
extern crate tokio;

use command_extra::CommandExtra;

// Bad: blessed unconditionally.
fn bare_command() {
    let mut command = std::process::Command::new("ls");
    command.arg("on-bare");
}

// Bad: blessed unconditionally.
fn boxed_command() {
    let mut command = Box::new(std::process::Command::new("ls"));
    command.arg("on-boxed");
}

// Bad: `tokio_process` is on, and tokio's `arg` is its own method rather
// than std's.
fn tokio_command() {
    let mut command = tokio::process::Command::new("ls");
    command.arg("on-tokio");
}

// Bad: a boxed tokio command, which `tokio_process` blesses.
fn boxed_tokio_command() {
    let mut command = Box::new(tokio::process::Command::new("ls"));
    command.arg("on-boxed-tokio");
}

// Not flagged: `async_process`'s impls are 1.5.0's, not this release's.
fn async_command() {
    let mut command = async_process::Command::new("ls");
    command.arg("on-async");
}

// Not flagged: the boxed `async_process` impl is 1.5.0's.
fn boxed_async_command() {
    let mut command = Box::new(async_process::Command::new("ls"));
    command.arg("on-boxed-async");
}

fn main() {}
