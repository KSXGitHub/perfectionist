// Shapes `mutating_command_builder` hands the fixer, for
// `tests/mutating_command_builder_autofix.rs`. This file and
// `applied.fixed.rs` differ by exactly the rewrite the fixer applies,
// and the test compares them byte for byte; `not_applied.rs` holds the
// shapes the rule declines to hand over at all.
//
// Each call carries a distinct argument so an assertion can name one
// shape without matching another.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use std::process::Command;

fn configure(_command: &mut Command) {}

// A discarded statement constrains nothing, so the rename stands alone.
pub fn discarded_statement() {
    Command::new("ls").arg("discarded-statement");
}

// The value is read, so the rewrite has to hand back the `&mut Command`
// the original returned.
pub fn argument_position() {
    configure(Command::new("ls").arg("argument-position"));
}

// The counterpart takes a concrete `Stdio` where the setter is generic
// over `Into<Stdio>`, so the argument carries the conversion too.
pub fn needs_a_conversion(file: std::fs::File) {
    configure(Command::new("ls").stdout(file));
}

pub struct Builder {
    pub command: Command,
}

pub fn make() -> Builder {
    Builder {
        command: Command::new("ls"),
    }
}

// A field of a value this expression produced: nothing else holds it.
pub fn field_of_a_temporary() {
    make().command.arg("field-of-a-temporary");
}
