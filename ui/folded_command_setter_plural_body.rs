// edition:2024
//
// A fold in the body of the plural itself. This crate stands in for
// `command-extra`, so the trait's default bodies are its own.

#![crate_name = "command_extra"]
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

pub mod caller {
    use super::{CommandExtra, Imitation, Imitator};
    use std::process::Command;

    // Bad: a fold outside the plural.
    pub fn lister(flags: &[&str]) -> Command {
        flags.iter().fold(Command::new("ls"), CommandExtra::with_arg)
    }

    // Not flagged: a trait of this crate other than `CommandExtra`,
    // though it declares the same pair.
    pub fn imitation(flags: std::vec::IntoIter<&'static str>) -> Imitator {
        flags.fold(Imitator, Imitation::with_arg)
    }
}

use std::ffi::OsStr;
use std::process::Command;

pub trait CommandExtra: Sized {
    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self;

    fn without_env(self, key: impl AsRef<OsStr>) -> Self;

    // Not flagged: the trait's default plural.
    fn with_args<Args>(self, args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>,
    {
        args.into_iter().fold(self, Self::with_arg)
    }

    // Not flagged: a fold in a closure inside the default plural.
    fn without_envs<Keys>(self, keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        let strip = |command| keys.into_iter().fold(command, Self::without_env);
        strip(self)
    }
}

impl CommandExtra for Command {
    fn with_arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.arg(arg);
        self
    }

    fn without_env(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env_remove(key);
        self
    }

    // Not flagged: an impl's own plural.
    fn without_envs<Keys>(self, keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        keys.into_iter()
            .fold(self, |command, key| command.without_env(key))
    }
}

pub struct Wrapped {
    inner: Command,
}

impl CommandExtra for Wrapped {
    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self {
        Wrapped {
            inner: self.inner.with_arg(arg),
        }
    }

    fn without_env(self, key: impl AsRef<OsStr>) -> Self {
        Wrapped {
            inner: self.inner.without_env(key),
        }
    }

    // Bad: an override folding another type's accumulator, whose plural
    // is that type's and so does not call this one.
    fn without_envs<Keys>(self, keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        Wrapped {
            inner: keys.into_iter().fold(self.inner, Command::without_env),
        }
    }
}

pub struct Tagged<'a> {
    inner: Command,
    tag: &'a str,
}

impl CommandExtra for Tagged<'_> {
    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self {
        Tagged {
            inner: self.inner.with_arg(arg),
            tag: self.tag,
        }
    }

    fn without_env(self, key: impl AsRef<OsStr>) -> Self {
        Tagged {
            inner: self.inner.without_env(key),
            tag: self.tag,
        }
    }

    // Not flagged: an override folding its own type, which names a
    // lifetime.
    fn with_args<Args>(self, args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>,
    {
        args.into_iter().fold(self, Self::with_arg)
    }
}

pub trait Imitation: Sized {
    fn with_arg(self, arg: &str) -> Self;

    fn with_args(self, args: &[&str]) -> Self;
}

pub struct Imitator;

impl Imitation for Imitator {
    fn with_arg(self, _arg: &str) -> Self {
        self
    }

    fn with_args(self, _args: &[&str]) -> Self {
        self
    }
}

fn main() {}
