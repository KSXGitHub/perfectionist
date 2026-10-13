// aux-build:command_extra.rs
// edition:2024
//
// Other traits that declare a method named like the plural.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// Bad: a type that another trait also gives a method named like the
// plural, so wherever both traits are in scope the rewrite is `E0034`.
// The fix is withheld.
pub struct Ambiguous;

impl CommandExtra for Ambiguous {
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

pub trait SameName {
    fn without_envs<Keys: IntoIterator>(self, keys: Keys) -> Self;
}

impl SameName for Ambiguous {
    fn without_envs<Keys: IntoIterator>(self, _keys: Keys) -> Self {
        self
    }
}

pub fn ambiguous(ambiguous: Ambiguous) -> Ambiguous {
    VARS.iter().fold(ambiguous, CommandExtra::without_env)
}

// Bad: another trait declares a method named like the plural, but on
// `&self`. Method resolution reaches the by-value plural first, so the
// fix is applied.
trait ByReference {
    fn without_envs<Keys: IntoIterator>(&self, keys: Keys) -> usize;
}

impl ByReference for Command {
    fn without_envs<Keys: IntoIterator>(&self, keys: Keys) -> usize {
        keys.into_iter().count()
    }
}

fn by_reference(command: Command) -> Command {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Bad: a type that another trait gives a method named like the plural
// taking `self: Box<Self>`, so for a boxed accumulator, wherever both
// traits are in scope, the rewrite is `E0034`. The fix is withheld.
struct BoxedAmbiguous;

impl CommandExtra for Box<BoxedAmbiguous> {
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

trait BoxedSameName {
    fn without_envs<Keys: IntoIterator>(self: Box<Self>, keys: Keys) -> Box<Self>;
}

impl BoxedSameName for BoxedAmbiguous {
    fn without_envs<Keys: IntoIterator>(self: Box<Self>, _keys: Keys) -> Box<Self> {
        self
    }
}

fn boxed_ambiguous(ambiguous: Box<BoxedAmbiguous>) -> Box<BoxedAmbiguous> {
    VARS.iter().fold(ambiguous, CommandExtra::without_env)
}

// No help names this trait, which nothing implements.
trait Unimplemented<Value> {
    fn without_envs(self, value: Value) -> Self;
}

// Bad: a generic trait, implemented twice for the accumulator, whose
// method named like the plural makes the rewrite ambiguous.
struct GenericallyAmbiguous;

impl CommandExtra for GenericallyAmbiguous {
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

trait GenericSameName<Value> {
    fn without_envs(self, value: Value) -> Self;
}

impl GenericSameName<u8> for GenericallyAmbiguous {
    fn without_envs(self, _value: u8) -> Self {
        self
    }
}

impl GenericSameName<u16> for GenericallyAmbiguous {
    fn without_envs(self, _value: u16) -> Self {
        self
    }
}

fn generically_ambiguous(ambiguous: GenericallyAmbiguous) -> GenericallyAmbiguous {
    VARS.iter().fold(ambiguous, CommandExtra::without_env)
}

fn main() {}
