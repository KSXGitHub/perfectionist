// aux-build:command_extra.rs
// edition:2024
//
// What the rule does and does not ask of the fold's accumulator. It
// never asks the accumulator's type: the folder resolving to a
// `CommandExtra` setter is what proves the accumulator implements the
// trait.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// Not flagged: an accumulator that is not a `Command`.
fn other_accumulator() {
    let _ = VARS.iter().fold(String::new(), |mut joined, var| {
        joined.push_str(var);
        joined
    });
}

// Not flagged: a fold over a std setter. The folder resolves to
// `Command::arg` rather than a `CommandExtra` setter, and its block
// holds a statement, so this is the sibling rule's to speak about.
#[expect(
    perfectionist::mutating_command_builder,
    reason = "the sibling rule owning this line is the point of the case"
)]
fn std_setter() {
    let _ = VARS.iter().fold(Command::new("ls"), |mut command, var| {
        command.arg(var);
        command
    });
}

// Bad: an accumulator the code names only by a type parameter. What
// proves the accumulator is a `CommandExtra` is the folder resolving to
// one of its setters, so the rule never asks the accumulator's type --
// which is also how it reaches whatever else a release implements the
// trait for.
fn generic_accumulator<Builder: CommandExtra>(command: Builder) -> Builder {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Not flagged: an accumulator that never arrives. The fold does not run,
// so the plural would strip an environment the fold left alone.
fn diverging_initial(flag: bool) -> Command {
    if flag {
        return VARS.iter().fold(return Command::new("ls"), CommandExtra::without_env);
    }
    Command::new("ls")
}

// Not flagged: the accumulator's own type carries an inherent method of
// the plural's name, which method resolution reaches before the trait's.
struct Shadowing(Command);

impl Shadowing {
    fn without_envs(self, _count: usize) -> Self {
        self
    }
}

impl CommandExtra for Shadowing {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

fn inherent_shadow(command: Shadowing) -> Shadowing {
    VARS.iter().fold(command, CommandExtra::without_env)
}

fn main() {}
