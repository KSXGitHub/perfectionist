// `command-extra` 1.5.0's `src/lib.rs` with one change: both its
// `async_process` and `tokio_process` gates are lifted, so every impl the
// crate can offer is present at once. compiletest cannot pass a cargo
// feature, and `ui/auxiliary/async_process_stub.rs` and
// `ui/auxiliary/tokio_stub.rs` stand in for the dependencies they name.
//
// The stand-ins are why the version-and-feature contrasts live in
// `tests/folded_command_setter_command_types.rs` instead, against the real
// crates. What this crate is for is a single fixture that can name every
// receiver the trait covers.
//
// A fixture using this crate has to name all three auxiliaries itself,
// stubs first: compiletest builds them in the order listed, into one
// directory it then puts on `-L`, and it does *not* read an `aux-build`
// directive out of an auxiliary file. Naming this one alone is `E0463`.

#![crate_name = "command_extra"]
extern crate async_process;
extern crate tokio;

use std::{
    borrow::Borrow,
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
    #[inline]
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
    #[inline]
    fn with_envs<Envs, Key, Value>(self, envs: Envs) -> Self
    where
        Envs: IntoIterator,
        Envs::Item: Borrow<(Key, Value)>,
        Key: AsRef<OsStr>,
        Value: AsRef<OsStr>,
    {
        envs.into_iter().fold(self, |cmd, pair| {
            let (key, value) = pair.borrow();
            cmd.with_env(key, value)
        })
    }

    /// Removes multiple environment variables.
    ///
    /// Equivalent to repeated [`Command::env_remove`]; no direct inherent method.
    #[inline]
    fn without_envs<Keys>(self, keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        keys.into_iter().fold(self, Self::without_env)
    }
}

macro_rules! impl_unit {
    ($(#[$attrs:meta])* $command:ty) => {
        $(#[$attrs])*
        impl CommandExtra for $command {
            #[inline]
            fn with_current_dir(mut self, dir: impl AsRef<Path>) -> Self {
                self.current_dir(dir);
                self
            }

            #[inline]
            fn with_env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
                self.env(key, value);
                self
            }

            #[inline]
            fn without_env(mut self, key: impl AsRef<OsStr>) -> Self {
                self.env_remove(key);
                self
            }

            #[inline]
            fn with_no_env(mut self) -> Self {
                self.env_clear();
                self
            }

            #[inline]
            fn with_arg(mut self, arg: impl AsRef<OsStr>) -> Self {
                self.arg(arg);
                self
            }

            #[inline]
            fn with_stdin(mut self, stdio: Stdio) -> Self {
                self.stdin(stdio);
                self
            }

            #[inline]
            fn with_stdout(mut self, stdio: Stdio) -> Self {
                self.stdout(stdio);
                self
            }

            #[inline]
            fn with_stderr(mut self, stdio: Stdio) -> Self {
                self.stderr(stdio);
                self
            }
        }
    };
}

macro_rules! impl_set {
    ($(#[$attrs:meta])* $base:ty) => {
        impl_unit!($(#[$attrs])* $base);
        impl_unit!($(#[$attrs])* Box<$base>);
        // no `&mut $base` because inherent methods already deal with it
    };
}

impl_set!(Command);
impl_set!(
    async_process::Command
);
impl_set!(
    tokio::process::Command
);
