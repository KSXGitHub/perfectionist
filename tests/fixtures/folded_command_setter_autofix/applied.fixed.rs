// Shapes `folded_command_setter` hands the fixer, for
// `tests/folded_command_setter_autofix.rs`. This header is shared
// between the pair: `applied.rs` holds the folds as written and
// `applied.fixed.rs` as the fixer leaves them, and the test compares
// the fixer's output against the latter byte for byte, so the two files
// have to differ by exactly the rewrite and by nothing else -- this
// header included.
//
// What that test asserts is the rule's own decision, not the
// compiler's, which is why it runs the fixer rather than reading a
// `.stderr`. `not_applied.rs` holds the shapes the rule declines to
// hand over at all.
//
// Each fold is one line, so the rewrite the fixer performs is a
// line-for-line swap and the pair stays legible side by side.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use std::collections::HashMap;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// A bare place expression: nothing to erase, and nothing to evaluate
// that the initial value could observe out of order.
pub fn bare_receiver(names: std::slice::Iter<'_, &'static str>) -> Command {
    let start = Command::new("ls");
    start.without_envs(names)
}

// `iter` on a reference, which std's `IntoIterator` for a reference
// agrees with, so the call is erased.
pub fn erased_iter() -> Command {
    let start = Command::new("ls");
    start.without_envs(VARS)
}

// `into_iter` is the very function the plural calls.
pub fn erased_into_iter(names: Vec<String>) -> Command {
    let start = Command::new("ls");
    start.without_envs(names)
}

// `iter` on an owned collection, which the fold only borrows, so
// erasing the call would move it and the `len` below would stop
// compiling. The call survives instead.
pub fn kept_iter(names: Vec<String>) -> (Command, usize) {
    let start = Command::new("ls");
    let command = start.without_envs(names.iter());
    (command, names.len())
}

// A closure forwarding to `with_arg`, whose plural is `with_args`.
pub fn forwarding_closure(flags: &[String]) -> Command {
    let start = Command::new("ls");
    start.with_args(flags)
}

// The `with_env` pair, whose item is an owned pair.
pub fn pair_item(pairs: Vec<(String, String)>) -> Command {
    let start = Command::new("ls");
    start.with_envs(pairs)
}

// The same pair over a map reference, whose items are pairs of
// references.
pub fn pair_item_borrowed(pairs: &HashMap<String, String>) -> Command {
    let start = Command::new("ls");
    start.with_envs(pairs)
}
