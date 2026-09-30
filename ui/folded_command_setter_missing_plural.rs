// aux-build:command_extra_1_0_0.rs
// edition:2024
//
// The plural has to exist in the `command-extra` the crate resolved.
// This fixture builds against 1.0.0, which has `with_args` but neither
// `with_envs` (1.1.0) nor `without_envs` (1.2.0), so the two env folds
// have no plural here to be named — where the sweep in
// `ui/folded_command_setter.rs`, built against 1.2.0, flags them.
//
// The `with_arg` fold is here to keep that silence attributable to the
// missing method rather than to the crate: the same shape, against the
// one plural this release does have.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// Not flagged: `without_envs` arrived in 1.2.0, so naming it here would
// suggest a call this crate cannot compile.
fn absent_plural() {
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: `with_envs` arrived in 1.1.0. The item is an owned pair
// rather than a reference to one, so the silence is the missing method
// and not the shape `with_envs` could not have taken anyway.
fn absent_plural_pair(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: `with_args` is a provided method of the trait as 1.0.0 declares
// it, and 1.0.0's own body is the fold this rule flags. A default body
// is still a method the suggestion can name.
fn present_plural() {
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::with_arg);
}

fn main() {}
