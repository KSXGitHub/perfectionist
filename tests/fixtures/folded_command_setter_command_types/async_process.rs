// Folds over `async_process::Command` and `Box<async_process::Command>`,
// for `tests/folded_command_setter_command_types.rs`. Both impls arrived
// in `command-extra` 1.5.0, behind its `async_process` feature, so this
// source compiles only on a release that has the feature, and only with
// the feature on.
//
// Neither const name is a substring of the other, so an assertion for
// one fold cannot be satisfied by the other fold's diagnostic.

#![allow(dead_code, unused_imports, reason = "fixture")]

use async_process::Command;
use command_extra::CommandExtra;

const BARE_ASYNC_VARS: &[&str] = &["A", "B"];
const BOXED_ASYNC_VARS: &[&str] = &["C", "D"];

pub fn async_command(command: Command) -> Command {
    BARE_ASYNC_VARS
        .iter()
        .fold(command, CommandExtra::without_env)
}

pub fn boxed_async_command(command: Box<Command>) -> Box<Command> {
    BOXED_ASYNC_VARS
        .iter()
        .fold(command, CommandExtra::without_env)
}
