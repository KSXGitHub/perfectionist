// A stand-in for `async_process::Command`, carrying only the setters
// `command_extra::CommandExtra` calls and with the signatures
// `async-process` gives them -- `&mut self -> &mut Command`, `Stdio` from
// std. Enough for an auxiliary `command_extra` to implement the trait for
// it, which the real crate does behind a feature compiletest cannot turn
// on.
//
// The crate name is `async_process` so that the impls read as they do
// upstream, and `Command` sits at the crate root rather than under a
// `process` module, which is where `async-process` puts it.

#![crate_name = "async_process"]
#![allow(dead_code, unused, reason = "aux fixture")]

use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

pub struct Command(std::process::Command);

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Command(std::process::Command::new(program))
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.0.arg(arg);
        self
    }

    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(&mut self, key: K, value: V) -> &mut Command {
        self.0.env(key, value);
        self
    }

    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Command {
        self.0.env_remove(key);
        self
    }

    pub fn env_clear(&mut self) -> &mut Command {
        self.0.env_clear();
        self
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Command {
        self.0.current_dir(dir);
        self
    }

    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.0.stdin(cfg);
        self
    }

    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.0.stdout(cfg);
        self
    }

    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.0.stderr(cfg);
        self
    }
}
