// aux-build:command_extra_split_bound.rs
// edition:2024
//
// A `with_envs` whose bound on the item is not a shape the rule reads.
// It cannot tell which items that plural accepts.

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

// Bad: a plural with no bound on a tuple, which the rule still names.
fn arguments(flags: Vec<String>) -> Command {
    flags.into_iter().fold(Command::new("ls"), CommandExtra::with_arg)
}

fn main() {}
