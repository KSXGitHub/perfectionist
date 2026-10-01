// aux-build:command_extra.rs
// edition:2024
//
// Which plural the trait declares, and what its bound accepts. What the
// fold proves of its item has to be what the plural asks of it, so the
// item's shape decides whether a plural can be named at all.
//
// Exercises `src/rules/folded_command_setter/setter.rs`.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];
const PAIRS: &[(&str, &str)] = &[("A", "1"), ("B", "2")];

// Bad: the `with_env` pair, whose item is a pair. No path spelling
// works, because `with_env` takes three arguments where `fold` supplies
// two. The closure destructures and forwards the bindings in order.
fn pair_item(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: the `with_env` pair over a map reference, whose items are pairs
// of references. The `iter` is std's, so the fix is applied.
fn pair_item_borrowed(pairs: &std::collections::HashMap<String, String>) {
    let _ = pairs
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Not flagged: a reference to a pair is not a pair. `with_envs` takes
// `IntoIterator<Item = (Key, Value)>`, and `&[(&str, &str)]` yields
// `&(&str, &str)`, which the fold binds through and the plural cannot.
fn reference_to_a_pair() {
    let _ = PAIRS
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Not flagged: a singular with no plural. Each sets one thing a later
// call replaces rather than extends.
fn no_plural() {
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, dir| c.with_current_dir(dir));
}

// Not flagged: a `ref` binding hands the setter a reference to the item
// where a by-value binding hands it the item, so what the fold proves of
// the item is not what the plural asks of it. Here the fold establishes
// `&Key: AsRef<OsStr>` and `without_envs` wants `Key: AsRef<OsStr>`.
fn ref_binding<Key>(command: Command, keys: Vec<Key>) -> Command
where
    for<'a> &'a Key: AsRef<OsStr>,
{
    keys.into_iter()
        .fold(command, |command, ref key| command.without_env(key))
}

// Not flagged: fewer arguments than the item has bindings. The plural
// would pass the whole pair where the fold passed one half of it.
fn dropped_binding(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, _value)| {
            command.without_env(key)
        });
}

// Not flagged: a one-element tuple wrapping a pair. The closure's
// bindings are the two the setter needs, but the *item* is a 1-tuple,
// which no plural can split.
fn nested_tuple_item(command: Command, pairs: Vec<((String, String),)>) -> Command {
    pairs
        .into_iter()
        .fold(command, |command, ((key, value),)| command.with_env(key, value))
}

// Not flagged: a tuple destructured for a setter that takes the item
// whole. The plural would be handed the tuple.
fn tuple_for_a_whole_item(command: Command, items: Vec<(String,)>) -> Command {
    items
        .into_iter()
        .fold(command, |command, (arg,)| command.with_arg(arg))
}

// Not flagged: a pair whose second half is destructured again. The
// plural would bind the 1-tuple where the fold bound what it wraps.
fn nested_pair(command: Command, items: Vec<(String, (String,))>) -> Command {
    items
        .into_iter()
        .fold(command, |command, (key, (value,))| command.with_env(key, value))
}

#[derive(Clone, Copy)]
struct PairCountdown(u8);

impl Iterator for PairCountdown {
    type Item = (&'static str, &'static str);

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = self.0.checked_sub(1)?;
        self.0 = remaining;
        Some(("A", "1"))
    }
}

// Bad: a `Copy` iterator of pairs behind a reference, which method
// resolution copies out for the fold. Its item is a pair once copied.
fn copied_pairs(command: Command, pairs: &PairCountdown) -> Command {
    pairs.fold(command, |command, (key, value)| command.with_env(key, value))
}

fn main() {}
