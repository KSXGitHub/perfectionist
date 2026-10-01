// aux-build:command_extra.rs
// edition:2024
//
// Which folders this rule matches, and which closure shapes count as
// one. The folder is matched by what it resolves to, so every spelling
// of the same setter is one shape; a closure earns the same treatment
// only where it forwards rather than computes.
//
// Exercises `src/rules/folded_command_setter/folder.rs`.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];
const PAIRS: &[(&str, &str)] = &[("A", "1"), ("B", "2")];

// Bad: the three pairs, spelled as a path. The receiver's `iter` comes
// from the standard library, so the fix is applied -- but the call
// survives into it, because only `into_iter` is erased.
fn paths() {
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::without_env);
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::with_arg);
}

// Bad: `without_env` reached through the concrete type and through a
// fully-qualified path — one method, so one shape.
fn spellings() {
    let _ = VARS.iter().fold(Command::new("ls"), Command::without_env);
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), <Command as CommandExtra>::without_env);
}

// Bad: `without_env` under a renamed import, in a module of its own. A
// `use` is module-wide, so at file scope the alias would be in scope for
// every case here and the trait would also be in scope under its own
// name. Only the alias is imported in this module, which is also what
// makes the case pin something: the trait counts as imported by what the
// `use` resolves to rather than by the name it binds. Whether that
// decides a fix is offered is `tests/folded_command_setter_autofix.rs`'s
// to say, since a `.stderr` cannot show applicability.
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
// remove it — it would move one call to the left and add a `map`.
fn transforming_closure() {
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, v| c.with_arg(format!("--{v}")));
}

// Not flagged: a closure whose block holds a statement, which is where
// a closure computes. A block that only wraps the forwarding call reads
// alike and is flagged — the `with_env` pairs above are written that
// way.
fn block_body() {
    let _ = VARS.iter().fold(Command::new("ls"), |command, var| {
        let key = var;
        command.without_env(key)
    });
}

// Not flagged: arguments passed out of the order they were bound.
fn swapped_arguments() {
    let _ = PAIRS
        .iter()
        .fold(Command::new("ls"), |c, (key, value)| c.with_env(value, key));
}

// Not flagged: a block holding a statement that is not a rebinding, so
// the `uses` check would accept the call while the statement is where
// the closure computes. The plural would delete it.
fn block_with_an_effect() {
    let _ = VARS.iter().fold(Command::new("ls"), |command, var| {
        println!("{var}");
        command.without_env(var)
    });
}

fn main() {}
