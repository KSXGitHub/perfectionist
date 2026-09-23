// `command-extra` 1.0.0's `src/lib.rs`, verbatim below the crate name.
// The release is real rather than a stand-in, so what it does not have
// is what a consumer pinning that version does not have: `with_envs`,
// added in 1.1.0, is the one counterpart the rule names that is absent.
//
// The crate name is set here rather than taken from the file name,
// because the rule looks for the trait under the name the published
// crate compiles as, and the file has to say which release it is.

#![crate_name = "command_extra"]

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Stdio},
};

pub trait CommandExtra: Sized {
    fn with_current_dir(self, dir: impl AsRef<Path>) -> Self;
    fn with_env(self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self;
    fn without_env(self, key: impl AsRef<OsStr>) -> Self;
    fn with_no_env(self) -> Self;
    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self;
    fn with_stdin(self, stdio: Stdio) -> Self;
    fn with_stdout(self, stdio: Stdio) -> Self;
    fn with_stderr(self, stdio: Stdio) -> Self;

    fn with_args<Args>(self, args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>,
    {
        args.into_iter().fold(self, |cmd, arg| cmd.with_arg(arg))
    }
}

impl CommandExtra for Command {
    fn with_current_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.current_dir(dir);
        self
    }

    fn with_env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env(key, value);
        self
    }

    fn without_env(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env_remove(key);
        self
    }

    fn with_no_env(mut self) -> Self {
        self.env_clear();
        self
    }

    fn with_arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.arg(arg);
        self
    }

    fn with_stdin(mut self, stdio: Stdio) -> Self {
        self.stdin(stdio);
        self
    }

    fn with_stdout(mut self, stdio: Stdio) -> Self {
        self.stdout(stdio);
        self
    }

    fn with_stderr(mut self, stdio: Stdio) -> Self {
        self.stderr(stdio);
        self
    }
}
