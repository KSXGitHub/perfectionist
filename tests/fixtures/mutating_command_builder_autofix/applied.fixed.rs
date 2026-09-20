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
    Command::new("ls").with_arg("discarded-statement");
}

// The value is read, so the rewrite has to hand back the `&mut Command`
// the original returned.
pub fn argument_position() {
    configure(&mut Command::new("ls").with_arg("argument-position"));
}

// The counterpart takes a concrete `Stdio` where the setter is generic
// over `Into<Stdio>`, so the argument carries the conversion too.
pub fn needs_a_conversion(file: std::fs::File) {
    configure(&mut Command::new("ls").with_stdout(file.into()));
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
    make().command.with_arg("field-of-a-temporary");
}

// Every link at once. Only the head is flagged -- each later one takes
// the `&mut Command` the previous returned -- but renaming the head
// alone would leave the rest calling std's against an owned receiver,
// so they move together. The trailing `status` takes `&mut self` and
// autorefs from the owned command, which is the form a person writes.
pub fn whole_chain() {
    let _ = Command::new("ls")
        .with_arg("chain-head")
        .with_arg("chain-tail")
        .status();
}

// And this is why they move together. With `Ext::arg` in scope, a
// rewrite that renamed only `current_dir` would leave `.arg` to be
// found on the owned command as `Ext::arg` rather than `Command::arg`
// -- compiling, and invisible in the diff. Renaming the whole chain
// takes the name out of play.
pub mod shadowed_next_link {
    use command_extra::CommandExtra;
    use std::process::Command;

    pub trait Ext {
        fn arg(self, value: &str) -> Self;
    }

    impl Ext for Command {
        fn arg(self, _value: &str) -> Self {
            self
        }
    }

    pub fn run() {
        let _ = Command::new("ls")
            .with_current_dir("/shadowed-next-link")
            .with_arg("shadowed-tail")
            .status();
    }
}
