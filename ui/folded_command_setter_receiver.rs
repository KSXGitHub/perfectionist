// aux-build:command_extra.rs
// edition:2024
//
// What the fold's receiver may be, and how much of it survives into the
// suggestion. The receiver has to be a place expression followed by at
// most one argument-less call, and that call is kept unless the plural
// makes it already.
//
// Exercises `src/rules/folded_command_setter/receiver.rs`.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

// Bad: a bare place expression as the receiver. Nothing to erase, and
// nothing to evaluate, so the fix is applied.
fn bare_receiver(names: std::slice::Iter<'_, &'static str>) {
    let _ = names.fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `into_iter` resolving to the trait method the plural calls
// itself, so the call is erased.
fn owned_into_iter(names: Vec<String>) {
    let _ = names
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `iter` on an owned collection, which the fold only borrows. The
// call survives, as every `iter` does.
fn owned_iter(names: Vec<String>) {
    let _ = names.iter().fold(Command::new("ls"), CommandExtra::without_env);
    let _ = names.len();
}

// Bad: a field reached through `self`, which is a place expression.
struct Config {
    names: Vec<String>,
}

impl Config {
    fn build(&self) -> Command {
        self.names
            .iter()
            .fold(Command::new("ls"), CommandExtra::without_env)
    }
}

// Not flagged: two calls on the receiver. The second is more text
// moving, so the rewrite would relocate rather than remove.
fn two_calls(names: Vec<String>) {
    let _ = names
        .into_iter()
        .rev()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: a call taking an argument, which is where logic hides.
fn call_with_argument(mut names: Vec<String>) {
    let _ = names
        .drain(1..)
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: `into_iter` as an associated-function call, which makes
// the receiver a call carrying an argument. A known syntactic gap, not a
// regression.
fn assoc_fn_receiver(names: Vec<String>) {
    let _ = Vec::into_iter(names).fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: a receiver the call did not run on. Method resolution derefs
// `&&Vec<String>` twice to reach `<[T]>::iter`, so the place is not the
// receiver, and handing the plural `v` would hand it something that is
// not an iterator at all. The call survives, so it compiles.
fn double_reference(names: &&Vec<String>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: a place the call did not run on, resolution derefing an `Rc` to
// reach `<[T]>::iter`.
fn through_rc(names: &std::rc::Rc<Vec<String>>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `into_iter` reached by an autoref, because the only
// `IntoIterator` is on the reference. Erasing would leave `Borrowed`,
// which is not an iterator, so the adjustment check keeps the call. The
// `into_iter` that runs is this crate's, so the fix is withheld.
struct Borrowed(Vec<String>);

impl<'a> IntoIterator for &'a Borrowed {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

fn autoref_into_iter(names: Borrowed) {
    let _ = names
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `iter` is std's by way of `Deref` while the type keeps an
// `IntoIterator` of its own that yields the other order. Erasing here
// would compile and silently reverse the arguments, which is why no
// `iter` is erased.
struct Backwards(Vec<String>);

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

fn disagreeing_into_iter(names: &Backwards) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: a `&mut` place, which erasing would move where the fold only
// borrowed.
fn mutable_reference(names: &mut Vec<String>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
    let _ = names.len();
}

// Bad: an `into_iter` whose iterator is a reference, which the fold
// reborrows. The call survives, because the reborrow is spelled on it:
// put on the place, `&mut *holder` would deref a type with no `Deref`.
struct Holder<'a>(&'a mut std::vec::IntoIter<String>);

impl<'a> IntoIterator for Holder<'a> {
    type Item = String;
    type IntoIter = &'a mut std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0
    }
}

fn reference_iterator(holder: Holder<'_>) {
    let _ = holder
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

fn main() {}
