// Shapes `folded_command_setter` fires on and declines to hand the
// fixer, for `tests/folded_command_setter_autofix.rs`. The test compares
// this file against itself, so anything the fixer rewrote fails it.
//
// Most are declined because the plural evaluates the initial value
// before the receiver where the fold evaluated the receiver first. Those
// would compile if they were rewritten, so the comparison fails if the
// fixer rewrites one. The trait out of scope is the exception: its
// rewrite would not compile, `cargo fix` would revert it, and the test's
// check for errors after applying fixes is what catches that instead.
//
// Each fold names a distinct program, so an assertion can name one
// shape without matching another.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use std::process::Command;

pub struct Weird(Vec<String>);

impl Weird {
    pub fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

// An `iter` of the linted crate's own, which promises nothing about
// agreeing with its own `IntoIterator`.
pub fn local_iter(weird: &Weird) -> Command {
    weird.iter().fold(Command::new("local-iter"), CommandExtra::without_env)
}

pub struct Shadow(Vec<String>);

impl Shadow {
    pub fn into_iter(self) -> std::vec::IntoIter<String> {
        self.0.into_iter()
    }
}

// An inherent `into_iter` shadowing the trait in method resolution.
pub fn shadowing_into_iter(shadow: Shadow) -> Command {
    shadow.into_iter().fold(Command::new("shadowed-into-iter"), CommandExtra::without_env)
}

pub struct Queue(Vec<String>);

impl Queue {
    pub fn take_all(&mut self) -> std::vec::IntoIter<String> {
        std::mem::take(&mut self.0).into_iter()
    }
}

// An argument-less call that mutates. The initial value here does not
// read what it changes, so this rewrite is safe and declined anyway --
// the gate is over-conservative on purpose.
pub fn mutating_receiver(queue: &mut Queue) -> Command {
    queue.take_all().fold(Command::new("mutating-receiver"), CommandExtra::without_env)
}

// `iter` reached through a `Deref` the user wrote, while the type keeps
// an `IntoIterator` yielding the other order. The `deref` body runs
// before the initial value in the fold and after it in the plural.
pub struct Backwards(Vec<String>);

impl std::ops::Deref for Backwards {
    type Target = Vec<String>;

    fn deref(&self) -> &Vec<String> {
        &self.0
    }
}

impl<'a> IntoIterator for &'a Backwards {
    type Item = &'a String;
    type IntoIter = std::iter::Rev<std::slice::Iter<'a, String>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().rev()
    }
}

pub fn user_deref(names: &Backwards) -> Command {
    names.iter().fold(Command::new("user-deref"), CommandExtra::without_env)
}

#[derive(Clone, Copy)]
pub struct Countdown(u8);

impl Iterator for Countdown {
    type Item = &'static str;

    fn next(&mut self) -> Option<&'static str> {
        let remaining = self.0.checked_sub(1)?;
        self.0 = remaining;
        Some("A")
    }
}

fn exhaust(countdown: &mut Countdown, program: &str) -> Command {
    countdown.0 = 0;
    Command::new(program)
}

// A place the initial value writes. The fold copies `countdown` before
// `exhaust` empties it, and the plural would copy it afterwards.
pub fn written_place(mut countdown: Countdown) -> Command {
    countdown.fold(exhaust(&mut countdown, "written-place"), CommandExtra::without_env)
}

// The plural is named rather than resolved, and this module does not
// import the trait -- a *path* folder needs no import of its own, so the
// fold compiles while the rewritten call would not. Advice only; the
// rewrite would be `E0599`.
pub mod trait_not_in_scope {
    use std::process::Command;

    const NOT_IN_SCOPE: &[&str] = &["a"];

    pub fn run(command: Command) -> Command {
        NOT_IN_SCOPE
            .iter()
            .fold(command, command_extra::CommandExtra::without_env)
    }
}
