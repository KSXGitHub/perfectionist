// A fold whose accumulator is a `Box<Command>`, for
// `tests/folded_command_setter_command_types.rs`. Whether this compiles
// at all depends on the `command-extra` resolved: 1.4.0 added
// `impl CommandExtra for Box<Command>`, and 1.3.0 has no such impl, so
// the same source is `E0277` there.
//
// The const's name appears in the diagnostic, which is how the test
// finds this fold's.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use std::process::Command;

const BOXED_VARS: &[&str] = &["A", "B"];

pub fn boxed(command: Box<Command>) -> Box<Command> {
    BOXED_VARS.iter().fold(command, CommandExtra::without_env)
}
