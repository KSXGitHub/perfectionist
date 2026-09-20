// Shapes `mutating_command_builder` hands the fixer, for
// `tests/mutating_command_builder_autofix.rs`. This header is shared
// between the pair: `applied.rs` holds the shapes as written and
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

// Every link at once. Only the head is flagged -- each later one takes
// the `&mut Command` the previous returned -- but they move together,
// for the reason `shadowed_next_link` shows. The trailing `status`
// takes `&mut self` and autorefs from the owned command, which is the
// form a person writes.
pub fn whole_chain() {
    let _ = Command::new("ls")
        .arg("chain-head")
        .arg("chain-tail")
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
            .current_dir("/shadowed-next-link")
            .arg("shadowed-tail")
            .status();
    }
}

// A macro invocation wrapping the head alone, so the `&mut ` goes in
// front of the invocation rather than the flagged call. The same shape
// is in `ui/mutating_command_builder_rewrite.rs`.
pub mod macro_wrapped_head {
    use command_extra::CommandExtra;
    use std::process::Command;

    macro_rules! passthrough {
        ($expression:expr) => {
            $expression
        };
    }

    fn configure(_command: &mut Command) {}

    pub fn run() {
        configure(passthrough!(Command::new("ls").arg("macro-head")).arg("macro-tail"));
    }
}
