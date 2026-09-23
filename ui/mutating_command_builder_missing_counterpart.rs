// aux-build:command_extra_1_0_0.rs
// edition:2024
//
// The counterpart has to exist in the `command-extra` the crate
// resolved. This fixture builds against 1.0.0, which does not have
// `with_envs`, so `Command::envs` has nothing here to be renamed to
// and the rule stays silent on it — where the sweep in
// `ui/mutating_command_builder.rs`, built against 1.2.0, flags it.
//
// The calls that do fire are here to keep the silence attributable to
// the missing method rather than to the crate: a required method, a
// provided one, and the pair whose two names have nothing in common,
// which is what would go silent were the trait asked for the std name.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

// Not flagged: `with_envs` arrived in 1.1.0, so naming it here would
// suggest a call this crate cannot compile.
fn absent_counterpart() {
    let mut command = Command::new("ls");
    command.envs([("LANG", "C")]);
}

// Bad: `with_env` is a required method of the trait as 1.0.0 declares
// it.
fn required_counterpart() {
    let mut command = Command::new("ls");
    command.env("LANG", "C");
}

// Bad: `with_args` is a provided method, whose body 1.0.0 folds over
// `with_arg`. A default body is still a method the call can reach.
fn provided_counterpart() {
    let mut command = Command::new("ls");
    command.args(["-a", "-h"]);
}

// Bad: `env_clear` and `with_no_env` share no part of a name, so the
// counterpart is what the trait is asked for, not the setter.
fn renamed_counterpart() {
    let mut command = Command::new("ls");
    command.env_clear();
}

// Not flagged: the chain is ported whole, and `with_envs` is not here
// to port `envs` to, so renaming `arg` around it would leave `envs`
// calling a std setter on a receiver the change had made owned. The
// head stands down with the link.
fn absent_counterpart_later_in_the_chain() {
    Command::new("ls").arg("-l").envs([("LANG", "C")]);
}

// Bad: the same shape with every counterpart present, which is what
// keeps the silence above attributable to the missing method rather
// than to the chain.
fn chain_of_present_counterparts() {
    Command::new("ls").arg("-l").env("LANG", "C");
}

fn main() {}
