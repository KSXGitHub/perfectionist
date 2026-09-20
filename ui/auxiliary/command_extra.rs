// A miniature stand-in for `command-extra`, declaring the by-value
// setters `mutating_command_builder` names in its diagnostics so a
// fixture can write the preferred form and still compile.
//
// The bodies are inert — each returns the command untouched. The rule
// only reads the *std* setters a fixture calls, so the stand-in needs
// the right signatures and nothing else. Inert bodies also keep the
// crate from tripping the very rule it supports: a faithful
// `fn with_arg(mut self, ...) -> Self { self.arg(arg); self }` calls a
// std setter on an owned `Command`, which is the trigger.
//
// The crate is named `command_extra` deliberately, rather than
// renamed on import the way `thiserror_stub` is: the rule's dependency
// gate looks for a declared dependency under exactly that name.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

pub trait CommandExtra: Sized {
    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self;
    fn with_args<Args>(self, args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>;
    fn with_env(self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self;
    fn with_envs<Envs, Key, Value>(self, envs: Envs) -> Self
    where
        Envs: IntoIterator<Item = (Key, Value)>,
        Key: AsRef<OsStr>,
        Value: AsRef<OsStr>;
    fn without_env(self, key: impl AsRef<OsStr>) -> Self;
    fn with_no_env(self) -> Self;
    fn with_current_dir(self, dir: impl AsRef<Path>) -> Self;
    fn with_stdin(self, stdio: Stdio) -> Self;
    fn with_stdout(self, stdio: Stdio) -> Self;
    fn with_stderr(self, stdio: Stdio) -> Self;
}

impl CommandExtra for Command {
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }

    fn with_args<Args>(self, _args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>,
    {
        self
    }

    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }

    fn with_envs<Envs, Key, Value>(self, _envs: Envs) -> Self
    where
        Envs: IntoIterator<Item = (Key, Value)>,
        Key: AsRef<OsStr>,
        Value: AsRef<OsStr>,
    {
        self
    }

    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }

    fn with_no_env(self) -> Self {
        self
    }

    fn with_current_dir(self, _dir: impl AsRef<Path>) -> Self {
        self
    }

    fn with_stdin(self, _stdio: Stdio) -> Self {
        self
    }

    fn with_stdout(self, _stdio: Stdio) -> Self {
        self
    }

    fn with_stderr(self, _stdio: Stdio) -> Self {
        self
    }
}
