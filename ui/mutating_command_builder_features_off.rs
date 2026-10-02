// aux-build:tokio_stub.rs
// aux-build:async_process_stub.rs
// aux-build:command_extra_1_4_0.rs
// edition:2024
//
// A release carrying the `Box` impl unconditionally and the `tokio` ones
// behind a feature that is off. Only what the build actually implements
// the trait for is reached, so the gated receivers are left alone even
// though the release has the code for them.
//
// This is the feature contrast a cargo feature cannot express here:
// compiletest cannot turn one on, so the vendored copy keeps the gate
// and the stubs supply the types the gated impls would name.

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
    command.arg("off-bare");
}

// Bad: blessed unconditionally too, which is what 1.4.0 added.
fn boxed_command() {
    let mut command = Box::new(std::process::Command::new("ls"));
    command.arg("off-boxed");
}

// Not flagged: the impl for this receiver is behind the feature.
fn tokio_command() {
    let mut command = tokio::process::Command::new("ls");
    command.arg("off-tokio");
}

// Not flagged: behind the same feature.
fn boxed_tokio_command() {
    let mut command = Box::new(tokio::process::Command::new("ls"));
    command.arg("off-boxed-tokio");
}

// Not flagged: this release has no impl for it under any feature.
fn async_command() {
    let mut command = async_process::Command::new("ls");
    command.arg("off-async");
}

// Not flagged: nor for its box.
fn boxed_async_command() {
    let mut command = Box::new(async_process::Command::new("ls"));
    command.arg("off-boxed-async");
}

fn main() {}
