// The shapes whose rewrite would compile, for
// `tests/mutating_command_builder_autofix.rs`; its sibling
// `rewrites_that_error.rs` holds the ones whose rewrite does not. They
// are alone in this crate on purpose, and that test's own docs say why:
// merging the two files back together would make its whole-file
// comparison unable to fail.

#![allow(dead_code, unused_imports, reason = "fixture")]

// Renaming `current_dir` makes the chain head an owned `Command`, and a
// by-value trait method is found before an inherent `&mut self` one, so
// the `.arg` below stops calling `Command::arg` and starts calling
// `Ext::arg`. That compiles, so there is no error to revert on, and
// nothing in the diff says what changed.
pub mod silent_redirect {
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

    pub fn chain() {
        let _ = Command::new("ls")
            .current_dir("/silent-redirect")
            .arg("redirect-tail")
            .status();
    }
}

// A discarded statement value constrains nothing, so the rename is the
// whole change and compiles.
pub mod statement_position {
    use command_extra::CommandExtra;
    use std::process::Command;

    pub fn run() {
        Command::new("ls").arg("statement-temporary");
    }
}

// A field of a value this expression produced: nothing else holds a
// claim on it, and the statement discards the result.
pub mod field_of_a_temporary {
    use command_extra::CommandExtra;
    use std::process::Command;

    pub struct Builder {
        pub command: Command,
    }

    pub fn make() -> Builder {
        Builder {
            command: Command::new("ls"),
        }
    }

    pub fn run() {
        make().command.arg("field-of-a-temporary");
    }
}
