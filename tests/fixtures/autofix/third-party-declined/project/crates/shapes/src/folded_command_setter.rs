//! A fold `perfectionist::folded_command_setter` reports and declines
//! to hand the fixer.
//!
//! Replacing a fold with the plural setter erases the call that
//! produced the iterator, which the rule will only do for a call it
//! recognises as the one the plural setter makes itself. An `iter` of
//! the crate's own is not that call, so the rewrite would drop code
//! of the author's.

use command_extra::CommandExtra;
use std::process::Command;

/// A sequence with an `iter` of its own.
pub struct Local(Vec<String>);

impl Local {
    /// A sequence of `names`.
    pub fn new(names: Vec<String>) -> Self {
        Local(names)
    }

    /// An `iter` the standard library never supplied.
    pub fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

/// A command with each name in `names` removed from its environment,
/// folded over an `iter` the plural setter would not call.
pub fn without_each(names: &Local) -> Command {
    let start = Command::new("ls");
    names.iter().fold(start, CommandExtra::without_env)
}

#[cfg(test)]
mod tests {
    /// The fold still removes every name it was given, so a rewrite
    /// of it would show up in the fixture's diff rather than here.
    #[test]
    fn the_fold_removes_every_name() {
        let names = super::Local::new(vec!["ALPHA".to_owned()]);
        assert_eq!(super::without_each(&names).get_envs().count(), 1);
    }
}
