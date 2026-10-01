// aux-build:command_extra_split_bound.rs
// edition:2024
//
// Plurals with bounds the rule does not read. It cannot tell which
// iterators those plurals accept.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

// Not flagged: whether the plural accepts the item is unknown.
fn pair_item(pairs: Vec<(String, String)>) -> Command {
    pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        })
}

// Not flagged: a plural asking for `Copy`, which the fold does not
// prove.
fn arguments(flags: Vec<String>) -> Command {
    flags.into_iter().fold(Command::new("ls"), CommandExtra::with_arg)
}

// Bad: a plural with only the bounds the fold proves, which the rule
// still names.
fn removals(keys: Vec<String>) -> Command {
    keys.into_iter().fold(Command::new("ls"), CommandExtra::without_env)
}

fn main() {}
