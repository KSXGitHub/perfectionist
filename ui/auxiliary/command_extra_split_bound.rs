// A `command-extra` whose `with_envs` bounds its item by a trait of its
// own rather than by a tuple, which is not a shape the rule reads. Only
// what the fixture reaches is declared, under the name the rule looks
// for.

#![crate_name = "command_extra"]

use std::ffi::OsStr;
use std::process::Command;

pub trait Split<Key, Value> {
    fn split(self) -> (Key, Value);
}

impl<Key, Value> Split<Key, Value> for (Key, Value) {
    fn split(self) -> (Key, Value) {
        self
    }
}

pub trait CommandExtra: Sized {
    fn with_env(self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self;

    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self;

    fn with_args<Args>(self, args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>,
    {
        args.into_iter().fold(self, Self::with_arg)
    }

    fn with_envs<Envs, Key, Value>(self, envs: Envs) -> Self
    where
        Envs: IntoIterator,
        Envs::Item: Split<Key, Value>,
        Key: AsRef<OsStr>,
        Value: AsRef<OsStr>,
    {
        envs.into_iter()
            .map(Split::split)
            .fold(self, |command, (key, value)| command.with_env(key, value))
    }
}

impl CommandExtra for Command {
    fn with_env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env(key, value);
        self
    }

    fn with_arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.arg(arg);
        self
    }
}
