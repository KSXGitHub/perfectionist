// `command-extra` 1.2.0's `src/lib.rs`, verbatim below this header.
// The crate is MIT-licensed and written by this repository's author,
// so copying it raises no licensing question, and a fixture writing
// the preferred form is then checked against the signatures a consumer
// actually gets rather than against a stand-in that may have drifted
// from them.
//
// The crate is named `command_extra` deliberately, rather than renamed
// on import the way `thiserror_stub` is: the rule's dependency gate
// looks for a declared dependency under exactly that name.
//
// The implementation is also the rule's own trigger — each `with_*`
// body calls a std setter on an owned `Command`, and there is nothing
// else such a body could do — but compiletest compares only the
// fixture's own output, so the diagnostics this crate earns as it
// builds reach no `.stderr` and need no `#[allow]`.

//! Builder-style methods for [`Command`] that take `self` and return `Self`.
//!
//! [std]'s own builder methods return `&mut Command`. They chain, but the chain
//! cannot produce a value, so a command that needs several settings has to be
//! built over a mutable binding and handed back separately. These methods
//! return the command itself, which puts the whole construction in expression
//! position: it can be returned, bound, stored in a field, or folded over.
//!
//! ```rust,no_run
//! # use command_extra::CommandExtra;
//! # use std::path::Path;
//! # use std::process::Command;
//! fn lister(dir: &Path) -> Command {
//!     Command::new("ls")
//!         .with_current_dir(dir)
//!         .with_args(["-l", "-a"])
//!         .with_env("LANG", "C")
//! }
//! ```
//!
//! The same function written against std cannot end in its chain, because the
//! chain has type `&mut Command`:
//!
//! ```rust,no_run
//! # use std::path::Path;
//! # use std::process::Command;
//! fn lister(dir: &Path) -> Command {
//!     let mut command = Command::new("ls");
//!     command
//!         .current_dir(dir)
//!         .args(["-l", "-a"])
//!         .env("LANG", "C");
//!     command
//! }
//! ```

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Stdio},
};

/// Builder-style methods for [`Command`] that take `self` and return `Self`.
pub trait CommandExtra: Sized {
    /// Sets the working directory.
    ///
    /// Corresponds to [`Command::current_dir`].
    fn with_current_dir(self, dir: impl AsRef<Path>) -> Self;

    /// Sets an environment variable.
    ///
    /// Corresponds to [`Command::env`].
    fn with_env(self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self;

    /// Removes an environment variable.
    ///
    /// Corresponds to [`Command::env_remove`].
    fn without_env(self, key: impl AsRef<OsStr>) -> Self;

    /// Clears all environment variables.
    ///
    /// Corresponds to [`Command::env_clear`].
    fn with_no_env(self) -> Self;

    /// Adds one argument.
    ///
    /// Corresponds to [`Command::arg`].
    fn with_arg(self, arg: impl AsRef<OsStr>) -> Self;

    /// Configures stdin.
    ///
    /// Corresponds to [`Command::stdin`].
    fn with_stdin(self, stdio: Stdio) -> Self;

    /// Configures stdout.
    ///
    /// Corresponds to [`Command::stdout`].
    fn with_stdout(self, stdio: Stdio) -> Self;

    /// Configures stderr.
    ///
    /// Corresponds to [`Command::stderr`].
    fn with_stderr(self, stdio: Stdio) -> Self;

    /// Adds multiple arguments.
    ///
    /// Corresponds to [`Command::args`].
    fn with_args<Args>(self, args: Args) -> Self
    where
        Args: IntoIterator,
        Args::Item: AsRef<OsStr>,
    {
        args.into_iter().fold(self, Self::with_arg)
    }

    /// Sets multiple environment variables.
    ///
    /// Corresponds to [`Command::envs`].
    fn with_envs<Envs, Key, Value>(self, envs: Envs) -> Self
    where
        Envs: IntoIterator<Item = (Key, Value)>,
        Key: AsRef<OsStr>,
        Value: AsRef<OsStr>,
    {
        envs.into_iter()
            .fold(self, |cmd, (key, value)| cmd.with_env(key, value))
    }

    /// Removes multiple environment variables.
    ///
    /// Equivalent to repeated [`Command::env_remove`]; no direct inherent method.
    fn without_envs<Keys>(self, keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        keys.into_iter().fold(self, Self::without_env)
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
