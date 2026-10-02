// aux-build:command_extra_1_4_0.rs
// edition:2024
//
// Which types carry `CommandExtra` is a property of the release. This
// fixture builds against 1.4.0, which added
// `impl CommandExtra for Box<Command>`, so a fold over a boxed command
// is flagged exactly as one over a bare command is.
//
// The other half of the contrast cannot be a fixture here, because
// without that impl the fold does not compile at all.
// `tests/folded_command_setter_command_types.rs` builds both releases
// and shows the `E0277`.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];
const PAIRS: &[(&str, &str)] = &[("A", "1"), ("B", "2")];

// Bad: a boxed accumulator, folder spelled as a path.
fn boxed_path(command: Box<Command>) -> Box<Command> {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Bad: a boxed accumulator, folder spelled as a forwarding closure.
fn boxed_closure(command: Box<Command>) -> Box<Command> {
    VARS.iter()
        .fold(command, |command, var| command.with_arg(var))
}

// Bad: the `with_env` pair over a boxed accumulator. The item is a
// reference to a pair, which 1.4.0's `with_envs` takes.
fn boxed_pair(command: Box<Command>) -> Box<Command> {
    PAIRS
        .iter()
        .fold(command, |command, (key, value)| command.with_env(key, value))
}

// Bad: a bare command, so the release's addition is shown to have taken
// nothing away.
fn bare(command: Command) -> Command {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Not flagged: the accumulator is a `Box` of something else entirely, so
// no setter of the trait resolves and there is nothing to name.
fn boxed_other(text: Box<String>) -> Box<String> {
    VARS.iter().fold(text, |mut text, var| {
        text.push_str(var);
        text
    })
}

fn main() {}
