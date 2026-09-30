// Folds over `tokio::process::Command` and `Box<tokio::process::Command>`,
// for `tests/folded_command_setter_command_types.rs`. Both impls live
// behind `command-extra`'s `tokio_process` feature, so this source
// compiles only when that feature is on, whatever the version.
//
// Each const is named distinctly so an assertion can name one fold
// without matching the other.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use tokio::process::Command;

const TOKIO_VARS: &[&str] = &["A", "B"];
const BOXED_TOKIO_VARS: &[&str] = &["C", "D"];

pub fn tokio_command(command: Command) -> Command {
    TOKIO_VARS.iter().fold(command, CommandExtra::without_env)
}

pub fn boxed_tokio_command(command: Box<Command>) -> Box<Command> {
    BOXED_TOKIO_VARS
        .iter()
        .fold(command, CommandExtra::without_env)
}
