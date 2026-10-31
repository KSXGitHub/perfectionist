//! A crate configuring a command through the mutating setters
//! `std::process::Command` offers, which
//! `perfectionist::mutating_command_builder` replaces with the
//! consuming setters of the same name from
//! [`command_extra`](https://docs.rs/command-extra).
//!
//! The trait is imported before the rewrite needs it: the rule only
//! offers the swap where the consuming setter is already in scope,
//! since bringing it into scope is not something a rewrite of the
//! call can do. A second function calls a consuming setter already,
//! so the import is in use before the rewrite: an import that only
//! the rewrite would use is one `cargo fix` removes in the same pass
//! as it applies the rename, and the rename then names a method that
//! is no longer in scope.

use command_extra::CommandExtra;
use std::process::Command;

/// A command carrying `label`, built with the consuming setter the
/// rule asks for.
pub fn already_consuming(label: &str) -> Command {
    Command::new("ls").with_arg(label)
}

/// How many arguments a command carries once it has been configured
/// in the expression that passes it on.
pub fn chained_arg_count() -> usize {
    count_args(Command::new("ls").arg("chained"))
}

/// The number of arguments `command` carries.
fn count_args(command: &mut Command) -> usize {
    command.get_args().count()
}

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("mutating_command_builder: {}\n", chained_arg_count())
}

#[cfg(test)]
mod tests {
    /// The argument still reaches the command, which swapping the
    /// mutating setter for the consuming one cannot change.
    #[test]
    fn the_argument_still_reaches_the_command() {
        assert_eq!(super::chained_arg_count(), 1);
    }

    /// The command built with the consuming setter carries the same
    /// one argument, so the two spellings answer alike.
    #[test]
    fn the_consuming_setter_carries_the_same_argument() {
        let command = super::already_consuming("chained");
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(arguments, [std::ffi::OsStr::new("chained")]);
    }
}
