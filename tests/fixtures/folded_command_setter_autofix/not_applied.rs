// Shapes `folded_command_setter` fires on and declines to hand the
// fixer, for `tests/folded_command_setter_autofix.rs`. The test compares
// this file against itself, so anything the fixer rewrote fails it.
//
// The rewrite is declined because the plural evaluates the initial value
// before the receiver where the fold evaluated the receiver first, and
// only a receiver whose call comes from the standard library is known
// not to care. Every shape here would compile if it were rewritten, so
// the comparison can fail rather than passing because `cargo fix`
// reverted the file.
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
