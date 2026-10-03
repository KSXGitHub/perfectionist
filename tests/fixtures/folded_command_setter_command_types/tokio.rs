// Folds over `tokio::process::Command` and `Box<tokio::process::Command>`,
// for `tests/folded_command_setter_command_types.rs`. Both impls live
// behind `command-extra`'s `tokio_process` feature, so this source
// compiles only when that feature is on, whatever the version.
//
// Neither const name is a substring of the other, so an assertion for
// one fold cannot be satisfied by the other fold's diagnostic.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use tokio::process::Command;

const BARE_TOKIO_VARS: &[&str] = &["A", "B"];
const BOXED_TOKIO_VARS: &[&str] = &["C", "D"];

pub fn tokio_command(command: Command) -> Command {
    BARE_TOKIO_VARS
        .iter()
        .fold(command, CommandExtra::without_env)
}

pub fn boxed_tokio_command(command: Box<Command>) -> Box<Command> {
    BOXED_TOKIO_VARS
        .iter()
        .fold(command, CommandExtra::without_env)
}
