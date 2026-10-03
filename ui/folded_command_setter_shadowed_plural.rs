// aux-build:command_extra.rs
// edition:2024
//
// Inherent methods named like the plural, which method resolution may
// reach before the trait's.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

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

// Bad: an inherent method of the plural's name that takes `&self`.
// Method resolution finds the by-value plural first, so the rewritten
// call reaches the trait's.
struct Borrowing(Command);

impl Borrowing {
    fn without_envs(&self, _count: usize) -> usize {
        0
    }
}

impl CommandExtra for Borrowing {
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

fn inherent_by_reference(command: Borrowing) -> Borrowing {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Bad: an inherent method of the plural's name on another instantiation
// of the accumulator's type, which method resolution never reaches for
// this one.
struct Wrapper<Inner>(Inner);

impl Wrapper<u32> {
    fn without_envs(self, _count: usize) -> Self {
        self
    }
}

impl<Inner> CommandExtra for Wrapper<Inner> {
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

fn other_instantiation(wrapped: Wrapper<String>) -> Wrapper<String> {
    VARS.iter().fold(wrapped, CommandExtra::without_env)
}

// Not flagged: an inherent method of the plural's name on the type a
// `Box` holds, taking the box by value. Method resolution reaches it
// before the trait's plural for the box.
struct Boxed;

impl Boxed {
    fn without_envs(self: Box<Self>, _count: usize) -> usize {
        0
    }
}

impl CommandExtra for Box<Boxed> {
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

fn boxed_receiver(boxed: Box<Boxed>) -> Box<Boxed> {
    VARS.iter().fold(boxed, CommandExtra::without_env)
}

// Bad: an accumulator whose `Deref` leads back to itself, which method
// resolution gives up on after its recursion limit, as does the rule.
struct Cyclic(Command);

impl std::ops::Deref for Cyclic {
    type Target = Cyclic;

    fn deref(&self) -> &Cyclic {
        self
    }
}

impl CommandExtra for Cyclic {
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

fn cyclic_deref(cyclic: Cyclic) -> Cyclic {
    VARS.iter().fold(cyclic, CommandExtra::without_env)
}

fn main() {}
