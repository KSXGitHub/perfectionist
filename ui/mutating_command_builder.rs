// aux-build:command_extra.rs
// edition:2024
//
// UI sweep for `mutating_command_builder` under the default
// configuration. Every std `Command` setter called on an owned command
// is flagged and offered its `CommandExtra` counterpart; a receiver
// that is already a borrow is left alone, because the by-value form
// cannot be reached from one.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::path::Path;
use std::process::{Command, Stdio};

// Bad: the whole table, each called on an owned `Command`.
fn every_setter(dir: &Path) {
    let mut command = Command::new("ls");
    command.arg("-l");
    command.args(["-a", "-h"]);
    command.env("LANG", "C");
    command.envs([("LANG", "C")]);
    command.env_remove("LANG");
    command.env_clear();
    command.current_dir(dir);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
}

// Bad: the shape the rule exists for. The chain cannot be the tail
// expression, so the settings spill into a statement over a `mut`
// binding that exists only until they are done.
fn lister(dir: &Path) -> Command {
    let mut command = Command::new("ls");
    command.current_dir(dir).args(["-l", "-a"]).env("LANG", "C");
    command
}

// Good: the same function as one expression.
fn lister_by_value(dir: &Path) -> Command {
    Command::new("ls")
        .with_current_dir(dir)
        .with_args(["-l", "-a"])
        .with_env("LANG", "C")
}

// Bad once, at the head of the chain: `Command::new(..)` is owned, so
// `.arg("a")` fires. The calls after it receive the `&mut Command` the
// previous one returned and are exempt.
fn chained() {
    Command::new("ls").arg("a").arg("b").status().ok();
}

// Not flagged: a borrowed receiver cannot adopt `CommandExtra`, whose
// methods take `self`. This is the exemption that matters — firing here
// would emit a diagnostic with no valid fix. Exempt rather than
// preferred: the rule has nothing better to offer, not an endorsement.
fn configure(command: &mut Command) {
    command.arg("-l");
    command.env("LANG", "C");
}

struct Builder {
    command: Command,
}

impl Builder {
    // Not flagged: `self.command` has type `Command` with no
    // reference in sight, but it sits behind `&mut self`, so the
    // by-value form cannot take it — E0507, cannot move out of a
    // place behind a mutable reference. The type alone does not
    // separate this case from the one below.
    fn extend(&mut self) {
        self.command.arg("-l");
    }

    // Bad: the same field, movable here because `self` is owned.
    fn into_extended(mut self) -> Command {
        self.command.arg("-l");
        self.command
    }

    // Good: the counterpart of the above. Taking the advice collapses
    // the two statements into one expression and drops the `mut`, since
    // the field is moved rather than mutated. The signature returns
    // `Command`, which is what `with_arg` hands back.
    fn into_extended_by_value(self) -> Command {
        self.command.with_arg("-l")
    }
}

// Not flagged: reached through a `Box`, so not the caller's to move
// out of.
fn boxed(command: &mut Box<Command>) {
    command.arg("-l");
}

// Not flagged: a closure parameter is a borrow like any other.
fn through_closure(command: &mut Command) {
    let mut add = |pending: &mut Command| {
        pending.arg("-l");
    };
    add(command);
}

// Not flagged: `Command::new` is not a setter, and the spawning methods
// have no by-value counterpart — they take `&mut self` legitimately.
fn spawning() {
    Command::new("ls").status().ok();
    Command::new("ls").output().ok();
    Command::new("ls").spawn().ok();
}

// Not flagged: a method of the same name on an unrelated type.
struct NotACommand;

impl NotACommand {
    fn arg(&mut self, _value: &str) {}
    fn current_dir(&mut self, _dir: &Path) {}
}

fn same_name_elsewhere() {
    let mut other = NotACommand;
    other.arg("-l");
    other.current_dir(Path::new("."));
}

// Good: the by-value form on an owned command is the shape the rule
// prefers, and the only kind of silence here that is an endorsement.
fn already_by_value(dir: &Path) -> Command {
    Command::new("ls")
        .with_arg("-l")
        .with_env("LANG", "C")
        .without_env("LC_ALL")
        .with_no_env()
        .with_current_dir(dir)
        .with_stdin(Stdio::null())
        .with_stdout(Stdio::null())
        .with_stderr(Stdio::null())
}

fn main() {}
