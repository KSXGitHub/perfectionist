// aux-build:tokio_stub.rs
// aux-build:command_extra_1_4_0_tokio.rs
// edition:2024
//
// What the diagnostic says for the two `tokio` accumulators 1.4.0
// implements the trait for. Whether they are reached at all is
// `tests/folded_command_setter_command_types.rs`, against the real
// crates and the real feature.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;
extern crate tokio;

use command_extra::CommandExtra;
use tokio::process::Command;

const VARS: &[&str] = &["A", "B"];

// Bad: a tokio command, which carries the trait behind the release's
// feature exactly as the std one carries it unconditionally.
fn tokio_command(command: Command) -> Command {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Bad: a boxed tokio command.
fn boxed_tokio_command(command: Box<Command>) -> Box<Command> {
    VARS.iter().fold(command, CommandExtra::with_arg)
}

fn main() {}
