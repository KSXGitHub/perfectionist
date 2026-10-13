// aux-build:command_extra.rs
// edition:2024
//
// What the rule does and does not ask of the fold's accumulator. It
// never asks whether the accumulator implements `CommandExtra`: the
// folder resolving to one of the trait's setters is what proves it does.

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

// Not flagged: a closure calling the std setter `Command::arg`, which is
// not a `CommandExtra` setter. `perfectionist::mutating_command_builder`
// reports it instead.
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

// Bad: a generic accumulator, which may override the plural, so the fix
// is withheld.
fn generic_accumulator<Builder: CommandExtra>(command: Builder) -> Builder {
    VARS.iter().fold(command, CommandExtra::without_env)
}

fn opaque() -> impl CommandExtra {
    Command::new("ls")
}

// Bad: an opaque accumulator, which may override the plural, so the fix
// is withheld.
fn opaque_accumulator() -> impl CommandExtra {
    VARS.iter().fold(opaque(), CommandExtra::without_env)
}

// Not flagged: an accumulator that never arrives. The fold does not run,
// so the plural would strip an environment the fold left alone.
fn diverging_initial(flag: bool) -> Command {
    if flag {
        return VARS.iter().fold(return Command::new("ls"), CommandExtra::without_env);
    }
    Command::new("ls")
}

// Bad: a type whose impl writes its own body for the plural. The
// override keeps the trait's signature, so the rewrite compiles, but it
// would run that body in place of the fold, and the fix is withheld.
pub struct Overriding;

impl CommandExtra for Overriding {
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
    fn without_envs<Keys>(self, _keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        self
    }
}

pub fn overriding(overriding: Overriding) -> Overriding {
    VARS.iter().fold(overriding, CommandExtra::without_env)
}

fn main() {}
