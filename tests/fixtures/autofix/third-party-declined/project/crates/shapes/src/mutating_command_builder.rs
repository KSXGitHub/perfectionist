//! Setter calls `perfectionist::mutating_command_builder` reports and
//! declines to hand the fixer.
//!
//! The rename alone turns the call's `&mut Command` into a `Command`,
//! which only holds where the receiver is a temporary the whole
//! construction can be written over. A receiver that is a binding a
//! later statement reads would need an edit at a span the rule was
//! never given.

use command_extra::CommandExtra;
use std::process::Command;

/// A command carrying `label`, built with the consuming setter, so
/// the trait is in use before any rewrite would need it.
pub fn already_consuming(label: &str) -> Command {
    Command::new("ls").with_arg(label)
}

/// How many arguments a command carries once it has been configured
/// over a binding that the next statement reads.
pub fn arg_count_over_a_binding() -> usize {
    let mut command = Command::new("ls");
    command.arg("over-a-binding");
    command.get_args().count()
}

#[cfg(test)]
mod tests {
    /// The argument still reaches the command, so a rewrite of the
    /// call would show up in the fixture's diff rather than here.
    #[test]
    fn the_argument_still_reaches_the_command() {
        assert_eq!(super::arg_count_over_a_binding(), 1);
        assert_eq!(super::already_consuming("x").get_args().count(), 1);
    }
}
