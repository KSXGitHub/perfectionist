// aux-build:command_extra_1_3_0.rs
// edition:2024
//
// What the `with_env` pair accepts is a property of the release, so the
// rule reads the plural's own bound rather than assuming one. This
// fixture builds against 1.3.0, whose `with_envs` takes
// `Envs::Item: Borrow<(Key, Value)>` — satisfied by a reference to a
// pair. The same folds against 1.2.0's `Item = (Key, Value)` are in
// `ui/folded_command_setter.rs`, where the two reference shapes are
// silent.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

const PAIRS: &[(&str, &str)] = &[("A", "1"), ("B", "2")];

// Bad: the item is a reference to a pair, which 1.2.0 rejected and
// 1.3.0 takes. Flagged here and silent in the 1.2.0 sweep.
fn reference_to_a_pair() {
    let _ = PAIRS
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: the item is a pair, which both releases take.
fn pair_item(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: a pair of references, which both releases take.
fn pair_of_references(pairs: &std::collections::HashMap<String, String>) {
    let _ = pairs
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Not flagged: a reference to a reference to a pair. `Borrow` reaches
// through one reference, not two, so the plural cannot take this even
// here — which the trait solver answers, rather than this rule counting
// reference layers itself.
fn reference_to_a_reference(pairs: Vec<&(String, String)>) {
    let _ = pairs
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: the two plurals whose bound 1.3.0 left alone, so that a
// regression in reading the loosened one does not go unnoticed here.
// Both take the item whole, so a reference to it satisfies `AsRef<OsStr>`
// and always did.
const VARS: &[&str] = &["A", "B"];

fn untouched_plurals(names: Vec<String>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::with_arg);
}

// Not flagged: the item is a reference to a pair whose elements are
// `AsRef<OsStr>` only *through* that reference. The fold binds `key` and
// `value` as references and so proves `&Key: AsRef<OsStr>`, where
// `with_envs` asks for `Key: AsRef<OsStr>` -- which does not follow, so
// the plural cannot take this iterator even here.
fn elements_borrowed_only<Key, Value>(command: Command, pairs: &[(Key, Value)]) -> Command
where
    for<'a> &'a Key: AsRef<std::ffi::OsStr>,
    for<'a> &'a Value: AsRef<std::ffi::OsStr>,
{
    pairs
        .iter()
        .fold(command, |command, (key, value)| command.with_env(key, value))
}

fn main() {}
