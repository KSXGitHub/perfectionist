// aux-build:command_extra.rs
// edition:2024
//
// Which folders this rule matches, and which closure shapes count as
// one. The folder is matched by what it resolves to, so every spelling
// of the same setter is one shape; a closure earns the same treatment
// only where it forwards rather than computes.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// Bad: the folder spelled as a path. The receiver's `iter` comes from
// the standard library, so the fix is applied. The call survives into
// it, because only `into_iter` is erased.
fn paths() {
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::without_env);
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::with_arg);
}

// Bad: `without_env` reached through the concrete type and through a
// fully-qualified path. One method, so one shape.
fn spellings() {
    let _ = VARS.iter().fold(Command::new("ls"), Command::without_env);
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), <Command as CommandExtra>::without_env);
}

// Bad: `without_env` under a renamed import, in a module of its own so
// that only the alias is in scope. The trait counts as imported by what
// the `use` resolves to, not by the name it binds, so no help asks for
// the import.
mod renamed_import {
    use super::VARS;
    use command_extra::CommandExtra as Ext;
    use std::process::Command;

    fn spelling() {
        let _ = VARS.iter().fold(Command::new("ls"), Ext::without_env);
    }
}

// Bad: a closure that forwards its parameters, written as a method call
// and as an associated-function call.
fn closures() {
    let _ = VARS.iter().fold(Command::new("ls"), |c, v| c.without_env(v));
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, v| CommandExtra::without_env(c, v));
}

// Not flagged: a closure that computes on the way. The plural would not
// remove it. It would move one call to the left and add a `map`.
fn transforming_closure() {
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, v| c.with_arg(format!("--{v}")));
}

// Not flagged: a closure whose block holds a statement, which is where
// a closure computes. A block that only wraps the forwarding call reads
// alike and is flagged.
fn block_body() {
    let _ = VARS.iter().fold(Command::new("ls"), |command, var| {
        let key = var;
        command.without_env(key)
    });
}

// Not flagged: arguments passed out of the order they were bound. The
// item is an owned pair, which the plural would take, so the order is
// what rules this out.
fn swapped_arguments(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |c, (key, value)| c.with_env(value, key));
}

// Not flagged: a block holding a statement with an effect, though the
// call it ends with forwards the parameters untouched. The plural would
// delete the statement.
fn block_with_an_effect() {
    let _ = VARS.iter().fold(Command::new("ls"), |command, var| {
        println!("{var}");
        command.without_env(var)
    });
}

// Not flagged: a `fold` of a trait other than `Iterator`. Its receiver
// need not iterate at all, so there is no plural to hand it.
struct Bag;

trait Gather {
    fn fold<Accumulator>(
        self,
        init: Accumulator,
        step: impl FnMut(Accumulator, &'static str) -> Accumulator,
    ) -> Accumulator;
}

impl Gather for Bag {
    fn fold<Accumulator>(
        self,
        init: Accumulator,
        mut step: impl FnMut(Accumulator, &'static str) -> Accumulator,
    ) -> Accumulator {
        step(init, "A")
    }
}

fn other_trait_fold(bag: Bag) -> Command {
    bag.fold(Command::new("ls"), CommandExtra::without_env)
}

// Not flagged: setters of another trait, though they share the names
// and the plural of `command_extra`'s.
trait Lookalike: Sized {
    fn without_env(self, key: &str) -> Self;

    fn without_envs(self, keys: &[&str]) -> Self;
}

struct Builder;

impl Lookalike for Builder {
    fn without_env(self, _key: &str) -> Self {
        self
    }

    fn without_envs(self, _keys: &[&str]) -> Self {
        self
    }
}

fn lookalike_setter(keys: std::vec::IntoIter<&'static str>) -> Builder {
    keys.fold(Builder, Lookalike::without_env)
}

// Not flagged: a trait of this crate's own that takes the published
// trait's name and its pair.
mod own_trait {
    pub trait CommandExtra: Sized {
        fn with_arg(self, arg: &str) -> Self;

        fn with_args(self, args: &[&str]) -> Self;
    }

    pub struct Own;

    impl CommandExtra for Own {
        fn with_arg(self, _arg: &str) -> Self {
            self
        }

        fn with_args(self, _args: &[&str]) -> Self {
            self
        }
    }

    pub fn own_trait(args: std::vec::IntoIter<&'static str>) -> Own {
        args.fold(Own, CommandExtra::with_arg)
    }
}

// Not flagged: a closure whose body is an `unsafe` block. The rewrite
// would drop the block along with whatever its author meant by it.
fn unsafe_block() {
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |command, var| unsafe { command.without_env(var) });
}

fn main() {}
