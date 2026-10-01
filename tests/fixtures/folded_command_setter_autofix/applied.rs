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
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

const VARS: &[&str] = &["A", "B"];

// A bare place expression: nothing to erase, and nothing to evaluate
// that the initial value could observe out of order.
pub fn bare_receiver(names: std::slice::Iter<'_, &'static str>) -> Command {
    let start = Command::new("ls");
    names.fold(start, CommandExtra::without_env)
}

// `iter` from the standard library, so the fix is applied. The call
// survives into it: no `iter` is erased, because a `Deref` can hand
// `iter` to std while the type keeps an `IntoIterator` of its own.
pub fn kept_iter_on_a_slice() -> Command {
    let start = Command::new("ls");
    VARS.iter().fold(start, CommandExtra::without_env)
}

// `into_iter` on the place's own type is the one call that is erased:
// the plural calls that very function on that very value.
pub fn erased_into_iter(names: Vec<String>) -> Command {
    let start = Command::new("ls");
    names.into_iter().fold(start, CommandExtra::without_env)
}

// The same over an owned collection the fold only borrows, with a later
// read to prove the borrow still ends where it did.
pub fn kept_iter_on_a_vec(names: Vec<String>) -> (Command, usize) {
    let start = Command::new("ls");
    let command = names.iter().fold(start, CommandExtra::without_env);
    (command, names.len())
}

// A closure forwarding to `with_arg`, whose plural is `with_args`.
pub fn forwarding_closure(flags: &[String]) -> Command {
    let start = Command::new("ls");
    flags.iter().fold(start, |command, flag| command.with_arg(flag))
}

// The `with_env` pair, whose item is an owned pair.
pub fn pair_item(pairs: Vec<(String, String)>) -> Command {
    let start = Command::new("ls");
    pairs.into_iter().fold(start, |command, (key, value)| command.with_env(key, value))
}

// The same pair over a map reference, whose items are pairs of
// references.
pub fn pair_item_borrowed(pairs: &HashMap<String, String>) -> Command {
    let start = Command::new("ls");
    pairs.iter().fold(start, |command, (key, value)| command.with_env(key, value))
}

// A place the call did not run on: resolution derefs `&&Vec<String>`
// twice to reach `<[T]>::iter`. Erasing would hand the plural something
// that is not an iterator, so the call has to survive for this to
// compile at all.
pub fn double_reference(names: &&Vec<String>) -> Command {
    let start = Command::new("ls");
    names.iter().fold(start, CommandExtra::without_env)
}

// An initial value that binds looser than a method call, which the
// suggestion has to bracket.
pub fn looser_initial(command: Box<Command>) -> Command {
    VARS.iter().fold(*command, CommandExtra::without_env)
}

// `into_iter` reached by an autoref, because the only `IntoIterator` is
// on the reference. Erasing would leave `PathBuf`, which is not an
// iterator, so the fixer reverting this file is how that regression
// would show up here.
pub fn autoref_into_iter(path: PathBuf) -> Command {
    let start = Command::new("ls");
    path.into_iter().fold(start, CommandExtra::with_arg)
}

// A field reached through an `Arc`, whose `deref` only projects a
// pointer.
pub struct Shared {
    vars: Vec<String>,
}

pub fn through_an_arc(shared: Arc<Shared>) -> Command {
    let start = Command::new("ls");
    shared.vars.iter().fold(start, CommandExtra::without_env)
}

// A receiver method resolution reborrowed for the fold, which the
// plural would otherwise move.
pub fn reborrowed(names: &mut std::vec::IntoIter<String>) -> (Command, usize) {
    let start = Command::new("ls");
    let command = names.fold(start, CommandExtra::without_env);
    (command, names.len())
}

#[derive(Clone, Copy)]
pub struct Countdown(u8);

impl Iterator for Countdown {
    type Item = &'static str;

    fn next(&mut self) -> Option<&'static str> {
        let remaining = self.0.checked_sub(1)?;
        self.0 = remaining;
        Some("A")
    }
}

// A `Copy` iterator behind a reference, which method resolution copies
// out for the fold, and which the plural would otherwise take as the
// reference.
pub fn copied_out(countdown: &Countdown) -> Command {
    let start = Command::new("ls");
    countdown.fold(start, CommandExtra::without_env)
}

pub struct Wrap {
    name: &'static str,
}

impl Wrap {
    fn build(self) -> Command {
        Command::new(self.name)
    }
}

// A struct literal at the head of a scrutinee, which the suggestion
// brackets so that it does not read as the start of the match's block.
pub fn literal_head() -> usize {
    match VARS.iter().fold(Wrap { name: "ls" }.build(), CommandExtra::without_env) {
        command => command.get_envs().count(),
    }
}

// A range, which HIR spells as a struct literal, and which needs no
// brackets.
pub fn range_initial() -> Command {
    VARS.iter().fold(Command::new(VARS[0..1][0]), CommandExtra::without_env)
}

// A renamed import, in a module of its own so the alias is the only
// `CommandExtra` in scope. The fixer rewriting this fold is the proof
// that the trait counts as imported by what the `use` resolves to rather
// than by the name it binds; were it the name, the suggestion would be
// advice and this file would come back unchanged.
pub mod renamed_import {
    use super::VARS;
    use command_extra::CommandExtra as Ext;
    use std::process::Command;

    pub fn spelling() -> Command {
        let start = Command::new("ls");
        VARS.iter().fold(start, Ext::without_env)
    }
}
