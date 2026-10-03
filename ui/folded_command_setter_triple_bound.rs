// aux-build:command_extra_triple_bound.rs
// edition:2024

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

// Not flagged: the plural would take only triples.
fn pair_item(pairs: Vec<(String, String)>) -> Command {
    pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        })
}

// Bad: a plural with only the bounds the fold proves, which the rule
// still names.
fn arguments(flags: Vec<String>) -> Command {
    flags.into_iter().fold(Command::new("ls"), CommandExtra::with_arg)
}

fn main() {}
