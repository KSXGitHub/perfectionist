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

// Bad: the feature is on, so this receiver is blessed and its `arg` is
// tokio's own method rather than std's.
fn tokio_command() {
    let mut command = tokio::process::Command::new("ls");
    command.arg("on-tokio");
}

// Bad: blessed by the same feature.
fn boxed_tokio_command() {
    let mut command = Box::new(tokio::process::Command::new("ls"));
    command.arg("on-boxed-tokio");
}

// Not flagged: the `async_process` impls arrived in a later release, so
// no feature of this one blesses this receiver.
fn async_command() {
    let mut command = async_process::Command::new("ls");
    command.arg("on-async");
}

// Not flagged: nor its box.
fn boxed_async_command() {
    let mut command = Box::new(async_process::Command::new("ls"));
    command.arg("on-boxed-async");
}

fn main() {}
