// A `command-extra` whose `with_envs` takes items that borrow as a
// triple, which no item a fold splits into a key and a value matches.
// Only what the fixture reaches is declared, under the name the rule
// looks for.

#![crate_name = "command_extra"]

use std::borrow::Borrow;
use std::ffi::OsStr;
use std::process::Command;

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

    fn with_envs<Envs, Key, Value, Extra>(self, envs: Envs) -> Self
    where
        Envs: IntoIterator,
        Envs::Item: Borrow<(Key, Value, Extra)>,
        Key: AsRef<OsStr>,
        Value: AsRef<OsStr>,
        Extra: AsRef<OsStr>,
    {
        envs.into_iter().fold(self, |command, item| {
            let (key, value, _) = item.borrow();
            command.with_env(key, value)
        })
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
