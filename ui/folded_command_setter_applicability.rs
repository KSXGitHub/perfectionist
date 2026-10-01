// aux-build:command_extra.rs
// edition:2024
//
// When the suggestion is a fix and when it is only advice. The rewrite
// moves the receiver past the initial value, so it is applied only
// where that cannot change what either of them sees.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// Bad: an `iter` of the linted crate's own. Flagged, but the fix is
// withheld: the call is not the standard library's, so what evaluating
// it does is unknown.
struct Weird(Vec<String>);

impl Weird {
    fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

fn local_iter(weird: Weird) {
    let _ = weird
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: an inherent `into_iter` shadowing the trait in method
// resolution. Flagged, call kept, fix withheld.
struct Shadow(Vec<String>);

impl Shadow {
    fn into_iter(self) -> std::vec::IntoIter<String> {
        self.0.into_iter()
    }
}

fn shadowing_into_iter(shadow: Shadow) {
    let _ = shadow
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: an argument-less call that mutates. The receiver test is
// structural, so this passes it and the rule fires; the fix is withheld
// because the initial value could observe the mutation in the new order.
struct Queue(Vec<String>);

impl Queue {
    fn drain_all(&mut self) -> std::vec::Drain<'_, String> {
        self.0.drain(..)
    }

    fn take_all(&mut self) -> std::vec::IntoIter<String> {
        std::mem::take(&mut self.0).into_iter()
    }
}

fn mutating_receiver(mut queue: Queue) {
    let _ = queue
        .drain_all()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: a mutating receiver against an initial value that reads what it
// changes. Advice only, and it stays advice however far the gate is
// widened: the two orders build different commands.
fn mutating_receiver_observed(mut queue: Queue) {
    let _ = queue.take_all().fold(
        Command::new(format!("ls{}", queue.0.len())),
        CommandExtra::without_env,
    );
}

// Bad: an initial value that binds looser than a method call, so the
// suggestion has to bracket it.
fn looser_initial(command: Box<Command>) {
    let _ = VARS.iter().fold(*command, CommandExtra::without_env);
}

struct Literal {
    inner: Command,
}

impl CommandExtra for Literal {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

// Bad: a struct-literal initial value where the call is a scrutinee.
// Unbracketed, `Literal { inner }.without_envs(..)` would read as the
// start of the match's block and not parse, so the suggestion brackets
// it.
fn struct_literal_initial(inner: Command) {
    match VARS.iter().fold(Literal { inner }, CommandExtra::without_env) {
        _ => {}
    }
}

// Not flagged: a fold inside a `macro_rules!` of this crate's own. The
// suggestion would replace the definition with text read from a call
// site, and two invocations would earn two suggestions at one span.
macro_rules! scrub {
    ($start:expr) => {
        VARS.iter().fold($start, CommandExtra::without_env)
    };
}

fn in_a_macro_body() {
    let start = Command::new("ls");
    let _ = scrub!(start);
}

// Not flagged: an overloaded `Deref` in the place. Reaching the place
// runs the `deref` body, and the suggestion moves that across the
// initial value.
struct Counted(Vec<String>);

struct Noisy(Counted);

impl std::ops::Deref for Noisy {
    type Target = Counted;

    fn deref(&self) -> &Counted {
        DEREFS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        &self.0
    }
}

impl<'a> IntoIterator for &'a Counted {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

static DEREFS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn overloaded_deref(noisy: Noisy) {
    let _ = (*noisy)
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: an overloaded `Deref` that method resolution inserts itself, with
// no `*` written. The initial value reads the counter the `deref` body
// bumps, so the two orders build different commands, and the fix is
// withheld.
fn overloaded_deref_by_autoderef(noisy: Noisy) {
    let _ = noisy.into_iter().fold(
        Command::new(format!(
            "ls{}",
            DEREFS.load(std::sync::atomic::Ordering::SeqCst)
        )),
        CommandExtra::without_env,
    );
}

// An iterator that is `Copy`, so a by-value receiver is copied when it
// is evaluated and keeps the value it had then.
#[derive(Clone, Copy)]
struct Countdown(u8);

impl Iterator for Countdown {
    type Item = &'static str;

    fn next(&mut self) -> Option<&'static str> {
        let remaining = self.0.checked_sub(1)?;
        self.0 = remaining;
        Some("A")
    }
}

fn exhaust(countdown: &mut Countdown) -> Command {
    countdown.0 = 0;
    Command::new("ls")
}

// Bad: a place the initial value writes. The fold copies `countdown`
// before `exhaust` empties it; the plural would copy it afterwards and
// fold over nothing.
fn place_written_by_the_initial(mut countdown: Countdown) -> Command {
    countdown.fold(exhaust(&mut countdown), CommandExtra::without_env)
}

// Bad: a place the initial value only reads. A read cannot change what
// the receiver copies, so the fix is applied.
fn place_read_by_the_initial(countdown: Countdown) -> Command {
    countdown.fold(
        Command::new(countdown.0.to_string()),
        CommandExtra::without_env,
    )
}

// Bad: the trait is not in scope at the call site, so the plural the
// suggestion names would not resolve. The receiver is a place whose call
// is std's, so the import is the only thing withheld here.
mod trait_not_in_scope {
    use super::VARS;
    use std::process::Command;

    fn fold(command: Command) -> Command {
        VARS.iter()
            .fold(command, command_extra::CommandExtra::without_env)
    }
}

fn main() {}
