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

// Bad: the stdio setters, both ways round. std takes anything
// `Into<Stdio>` where the by-value form takes a concrete `Stdio`, so an
// argument that is already a `Stdio` renames straight across, and one
// that is merely convertible needs `.into()` — which the diagnostic
// says. The convertible one is also where the `.into()` advice meets
// the receiver line, the pair having no other home.
fn stdio_setters(file: std::fs::File) {
    let mut command = Command::new("ls");
    command.stdin(Stdio::null());
    command.stdout(file);
    command.stderr(Stdio::piped());
}

// Bad: a discarded statement value constrains nothing, so turning the
// `&mut Command` into a `Command` is the whole change here and the
// rewrite is applied. No "receiver outlives this call" line either,
// since the receiver is a temporary and there is no binding to
// reassign.
fn statement_position() {
    Command::new("ls").arg("statement-temporary");
}

// Bad: the receiver is a temporary too, but its value is a call
// argument, and `configure` wants the `&mut Command` the std setter
// returns. The rewrite puts that borrow back with a `&mut ` in front of
// the chain, so the argument keeps the type it had.
fn argument_position(dir: &Path) {
    configure(Command::new("ls").current_dir(dir));
}

// Bad: both reasons at once, and the later read is what makes the
// receiver line's condition hold. `command` is moved by the rename and
// read afterwards, and the value goes to `configure`, which wanted the
// `&mut Command`.
// Neither option the receiver line names is enough on its own here,
// since both are `E0308` until the call site takes the borrow, which is
// why that line says the change "also has to" rather than that it
// finishes there.
fn both_reasons(dir: &Path) {
    let mut command = Command::new("ls");
    configure(command.current_dir(dir));
    command.status().ok();
}

// Bad: a temporary feeding a method receiver, where the counterpart's
// argument needs the `.into()` the std setter did for itself. The
// conversion is part of the same rewrite, and the trailing `.status()`
// takes the owned command by autoref, so nothing after the chain has to
// move.
fn stdio_on_a_temporary(file: std::fs::File) {
    Command::new("ls").stdout(file).status().ok();
}

// Bad: an empty `::<>` names no generic argument, so the counterpart
// cannot disagree about one and the rewrite is applied. The token alone
// is not what withholds it.
fn empty_turbofish() {
    Command::new("ls").env_clear::<>();
}

// Bad: `with_envs` happens to take the same three generic parameters,
// so this turbofish would survive the rename; the guard, though, is one
// predicate over the whole table, and the generics line below has to
// avoid claiming the counterpart's set differs. Advice plus that line.
fn turbofished_envs() {
    Command::new("ls").envs::<[(&str, &str); 1], &str, &str>([("LANG", "C")]);
}

// Bad: the turbofish is written against `args`' two generic parameters
// and survives a rename of the segment alone, where `with_args` takes
// one, which is `E0107`. Advice plus the generics line.
fn turbofished() {
    Command::new("ls")
        .args::<[&str; 1], &str>(["-l"])
        .status()
        .ok();
}

// The generic-arguments line, the receiver line and the position line
// can each join the advice, and the combinations are what a reader
// actually meets. Between these and the fixtures above, every
// combination of them appears under each form of the advice, because a
// line that re-opens what another has settled is only visible side by
// side.

// Bad: an argument needing `.into()`, and a position that wanted the
// borrow. Both belong to the one rewrite, which is where the `&mut `
// and the `.into()` appear together.
fn conversion_and_position(file: std::fs::File) {
    configure(Command::new("ls").stdout(file));
}

// Bad: an argument needing `.into()` and a turbofish. Advice plus the
// generic-arguments line, whose check is a quick one here, since the
// stdio counterparts take a concrete `Stdio` and so have no generic
// parameter at all.
fn conversion_and_turbofish(file: std::fs::File) {
    Command::new("ls").stdout::<std::fs::File>(file);
}

// Bad: a turbofish over a binding. Advice plus the generic-arguments
// line plus the receiver line.
fn turbofish_and_receiver() {
    let mut command = Command::new("ls");
    command.args::<[&str; 1], &str>(["-l"]);
}

// Bad: a turbofish on a temporary whose value wanted the borrow. Advice
// plus the generic-arguments line plus the position line.
fn turbofish_and_position() {
    configure(Command::new("ls").args::<[&str; 1], &str>(["-l"]));
}

// Bad: an argument needing `.into()` on a binding whose value then
// wanted the borrow. Advice plus the receiver line plus the position
// line.
fn conversion_and_receiver_and_position(file: std::fs::File) {
    let mut command = Command::new("ls");
    configure(command.stdout(file));
}

// Bad: an argument needing `.into()`, a turbofish, and a binding.
fn conversion_and_turbofish_and_receiver(file: std::fs::File) {
    let mut command = Command::new("ls");
    command.stdout::<std::fs::File>(file);
}

// Bad: an argument needing `.into()`, a turbofish, and a position that
// wanted the borrow.
fn conversion_and_turbofish_and_position(file: std::fs::File) {
    configure(Command::new("ls").stdout::<std::fs::File>(file));
}

// Bad: all three lines beside the plain advice.
fn turbofish_and_receiver_and_position() {
    let mut command = Command::new("ls");
    configure(command.args::<[&str; 1], &str>(["-l"]));
}

// Bad: all three beside the `.into()` advice, which is every line the
// diagnostic can carry short of a remedy.
fn conversion_and_turbofish_and_receiver_and_position(file: std::fs::File) {
    let mut command = Command::new("ls");
    configure(command.stdout::<std::fs::File>(file));
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

// Bad: once, at the head of the chain. `Command::new(..)` is owned, so
// `.arg("a")` fires; the calls after it receive the `&mut Command` the
// previous one returned and are exempt. The rewrite renames every link
// all the same, since renaming the head alone would leave the rest
// calling std setters on a receiver that is now owned.
fn chained() {
    Command::new("ls").arg("a").arg("b").status().ok();
}

// Not flagged: `CommandExtra`'s methods take `self`, and a borrowed
// receiver has none to give. `std::mem::take` cannot buy one either,
// since `Command` has no `Default`; `std::mem::replace` can, at the
// cost of a placeholder command and a write-back around the settings,
// which is more code than the borrowed form it would replace. Exempt
// rather than preferred: the rule has nothing better to offer, not an
// endorsement.
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
    // by-value form cannot take it — `E0507`, cannot move out of a
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
    // the statements into one chained expression and drops the `mut`,
    // since the field is moved rather than mutated. The signature returns
    // `Command`, which is what `with_arg` hands back.
    fn into_extended_by_value(self) -> Command {
        self.command.with_arg("-l")
    }
}

fn make_builder() -> Builder {
    Builder {
        command: Command::new("ls"),
    }
}

// Bad: the receiver is a field of a value this expression produced, so
// nothing else holds a claim on it and the rewrite is applied.
//
// The call is also inert: the command it configures is dropped at the
// semicolon without ever being run. The rule neither notices that nor
// needs to, since it reads the shape of the call rather than what the
// program does with it, and `field_of_a_temporary_that_runs` below is
// the same shape spending its command.
fn field_of_a_temporary() {
    make_builder().command.arg("field-of-a-temporary");
}

// Bad: that shape with the command actually spent. The chain is
// rewritten whole, and `.status()` takes the owned command by autoref,
// which is the form the author would have written by hand.
fn field_of_a_temporary_that_runs() {
    make_builder().command.arg("runs-it").status().ok();
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
// base, for the reason `src/rules/mutating_command_builder/receiver.rs`
// gives.
fn indexed(mut commands: Vec<Command>) {
    commands[0].arg("indexed-vec");
    [Command::new("ls")][0].arg("indexed-array");
}

// Not flagged: the setter is written in a macro body, so the only span
// the diagnostic could point at is a single token in the definition, and
// one diagnostic would arrive per invocation.
macro_rules! add_arg {
    ($command:expr) => {
        $command.arg("in-a-macro-body")
    };
}

fn through_a_macro() {
    let mut command = Command::new("ls");
    add_arg!(command).status().ok();
}

// Bad: with the remedy that says to import the trait. The crate is a
// declared dependency, and the root imports it, but this module does
// not, so writing the counterpart here would be `no method named
// with_arg found` until the `use` arrives.
mod trait_not_imported {
    use std::process::Command;

    fn without_the_import() {
        let mut command = Command::new("ls");
        command.arg("no-import-here");
    }

    // Bad, and still no rewrite: an import does not make a reordering
    // safe, so the hazard is asked before the counterpart's
    // availability. Rendering the rename here and then telling the
    // reader to fetch the import would hand them the edit the rule
    // just refused.
    fn a_hazard_outlives_a_missing_import() {
        struct Noisy;

        impl Drop for Noisy {
            fn drop(&mut self) {}
        }

        impl AsRef<std::ffi::OsStr> for Noisy {
            fn as_ref(&self) -> &std::ffi::OsStr {
                std::ffi::OsStr::new("no-import-hazard")
            }
        }

        Command::new("ls").arg(&Noisy);
    }

    // Bad: the rename is shown beside the remedy, which rides in the
    // rename's own message. `src/rules/mutating_command_builder/emit.rs`
    // says why it has to.
    fn rename_beside_the_remedy() {
        Command::new("ls").arg("rename-plus-remedy");
    }
}

// Bad: the statement that discards the value is written in the macro
// body, and the expression in it is the caller's, so one span serves
// both uses, and a rename shown for the discarded one would be written
// over the borrow `configure` takes as well. Advice plus the position
// line.
macro_rules! discard_then_borrow {
    ($command:expr) => {{
        $command;
        configure($command);
    }};
}

#[expect(
    perfectionist::impure_macro_arguments,
    reason = "the argument has to be a temporary for the receiver check to reach the position check"
)]
fn macro_reuses_the_expression(dir: &Path) {
    discard_then_borrow!(Command::new("ls").current_dir(dir));
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

// Not flagged: a type of the author's own that is *also* called
// `Command`, with setters spelled exactly as std's and returning the
// same borrow. The rule asks for the `rustc_diagnostic_item` std's
// `Command` carries, which no local type has, so matching the name and
// the signatures buys nothing.
mod shadowing_command {
    use std::path::Path;

    struct Command;

    impl Command {
        fn new(_program: &str) -> Self {
            Command
        }

        fn arg(&mut self, _value: &str) -> &mut Self {
            self
        }

        fn current_dir(&mut self, _dir: &Path) -> &mut Self {
            self
        }
    }

    fn same_name_same_shape() {
        let mut command = Command::new("ls");
        command.arg("-l");
        Command::new("ls")
            .arg("shadowing-command")
            .current_dir(Path::new("."));
    }
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
