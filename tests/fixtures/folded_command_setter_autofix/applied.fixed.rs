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
    start.without_envs(names)
}

// `iter` from the standard library, so the fix is applied. The call
// survives into it: no `iter` is erased, because a `Deref` can hand
// `iter` to std while the type keeps an `IntoIterator` of its own.
pub fn kept_iter_on_a_slice() -> Command {
    let start = Command::new("ls");
    start.without_envs(VARS.iter())
}

// `into_iter` on the place's own type is the one call that is erased:
// the plural calls that very function on that very value.
pub fn erased_into_iter(names: Vec<String>) -> Command {
    let start = Command::new("ls");
    start.without_envs(names)
}

// The same over an owned collection the fold only borrows, with a later
// read to prove the borrow still ends where it did.
pub fn kept_iter_on_a_vec(names: Vec<String>) -> (Command, usize) {
    let start = Command::new("ls");
    let command = start.without_envs(names.iter());
    (command, names.len())
}

// A closure forwarding to `with_arg`, whose plural is `with_args`.
pub fn forwarding_closure(flags: &[String]) -> Command {
    let start = Command::new("ls");
    start.with_args(flags.iter())
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
    start.with_envs(pairs.iter())
}

// A place the call did not run on: resolution derefs `&&Vec<String>`
// twice to reach `<[T]>::iter`. Erasing would hand the plural something
// that is not an iterator, so the call has to survive for this to
// compile at all.
pub fn double_reference(names: &&Vec<String>) -> Command {
    let start = Command::new("ls");
    start.without_envs(names.iter())
}

// An initial value that binds looser than a method call, which the
// suggestion has to bracket.
pub fn looser_initial(command: Box<Command>) -> Command {
    (*command).without_envs(VARS.iter())
}

// `into_iter` reached by an autoref, because the only `IntoIterator` is
// on the reference. Erasing would leave `PathBuf`, which is not an
// iterator, so the fixer reverting this file is how that regression
// would show up here.
pub fn autoref_into_iter(path: PathBuf) -> Command {
    let start = Command::new("ls");
    start.with_args(path.into_iter())
}

// A field reached through an `Arc`, whose `deref` only projects a
// pointer.
pub struct Shared {
    vars: Vec<String>,
}

pub fn through_an_arc(shared: Arc<Shared>) -> Command {
    let start = Command::new("ls");
    start.without_envs(shared.vars.iter())
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
        start.without_envs(VARS.iter())
    }
}
