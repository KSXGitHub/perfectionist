// A fold whose accumulator is a `Box<Command>`, for
// `tests/folded_command_setter_command_types.rs`. Whether this compiles
// at all depends on the `command-extra` resolved: 1.4.0 added
// `impl CommandExtra for Box<Command>`, and 1.3.0 has no such impl, so
// the same source is `E0277` there.
//
// The const is named distinctly so an assertion can find this fold's
// diagnostic without matching another file's.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use std::process::Command;

const BOXED_VARS: &[&str] = &["A", "B"];

pub fn boxed(command: Box<Command>) -> Box<Command> {
    BOXED_VARS.iter().fold(command, CommandExtra::without_env)
}
