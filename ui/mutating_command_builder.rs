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
}

// Bad, both ways round. std takes anything `Into<Stdio>` where the
// by-value form takes a concrete `Stdio`, so an argument that is
// already a `Stdio` renames straight across, and one that is merely
// convertible needs `.into()` — which the diagnostic says.
fn stdio_setters(file: std::fs::File) {
    let mut command = Command::new("ls");
    command.stdin(Stdio::null());
    command.stdout(file);
    command.stderr(Stdio::piped());
}

// Bad, and the rename is shown: a discarded statement value constrains
// nothing, so turning the `&mut Command` into a `Command` is the whole
// change here. No "receiver outlives this call" line either -- the
// receiver is a temporary, and there is no binding to reassign.
fn statement_position() {
    Command::new("ls").arg("statement-temporary");
}

// Bad, advice only: the receiver is a temporary too, but its value is a
// call argument, and `configure` wants the `&mut Command` the std setter
// returns.
fn argument_position(dir: &Path) {
    configure(Command::new("ls").current_dir(dir));
}

// Bad, advice only, and the receiver is not the reason: a temporary
// feeding a method receiver, which on its own earns the rename. What
// withholds it is the `.into()` the counterpart's argument needs, since
// a rename plus a conversion is not a rename.
fn stdio_on_a_temporary(file: std::fs::File) {
    Command::new("ls").stdout(file).status().ok();
}

// Bad, advice only: the turbofish is written against `args`' two
// generic parameters and survives a rename of the segment alone, where
// `with_args` takes one -- `E0107`.
fn turbofished() {
    Command::new("ls")
        .args::<[&str; 1], &str>(["-l"])
        .status()
        .ok();
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
        self.command.arg("borrowed-field");
    }

    // Bad: the same field, movable here because `self` is owned.
    fn into_extended(mut self) -> Command {
        self.command.arg("owned-field");
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

// Not flagged: the receiver's type is `&mut Box<Command>`, which is not
// `Command`, so this stops at the receiver-type check rather than at the
// place walk. Kept because `Box` is the shape a reader expects to see
// covered.
fn boxed(command: &mut Box<Command>) {
    command.arg("-l");
}

// Not flagged: moving a field out of a value that implements `Drop` is
// `E0509`, so there is no way for the author to finish the fix.
struct Dropper {
    command: Command,
}

impl Drop for Dropper {
    fn drop(&mut self) {}
}

impl Dropper {
    fn use_it(mut self) {
        self.command.arg("drop-field");
    }
}

// Not flagged: the binding belongs to the enclosing body, so the
// closure only borrows it. Moving out of an upvar is `E0507`, and
// taking it by value would turn this `FnMut` into an `FnOnce`.
fn captured_upvar() {
    let mut command = Command::new("ls");
    let mut go = || {
        command.arg("captured-upvar");
    };
    go();
    go();
}

// Not flagged: the parameter's type is `&mut Command`, so like
// `configure` above this stops at the receiver-type check. The
// captured-binding case, which does reach the place walk, is
// `captured_upvar`.
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

// Not flagged: moving out of an index is never allowed, whatever the
// base -- `Index` hands back a borrow (`E0507`), and an array index
// moves out of a non-copy array (`E0508`).
fn indexed(mut commands: Vec<Command>) {
    commands[0].arg("indexed-vec");
    [Command::new("ls")][0].arg("indexed-array");
}

// Not flagged: an extension trait taking `self` is found at the
// by-value step of the autoderef chain, before `Command`'s own
// `&mut self` setter, so this resolves to `Ext::arg` and renaming it
// would replace a method of the author's own.
mod extension_trait {
    use std::process::Command;

    trait Ext {
        fn arg(self, value: &str) -> Self;
    }

    impl Ext for Command {
        fn arg(self, _value: &str) -> Self {
            self
        }
    }

    fn via_extension_trait() {
        let _: Command = Command::new("ls").arg("ext-trait");
    }
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
