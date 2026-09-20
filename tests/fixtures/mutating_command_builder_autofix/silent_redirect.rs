// The one rewrite of this rule's that would compile, for
// `tests/mutating_command_builder_autofix.rs`. It is alone in its
// fixture crate on purpose: `cargo fix` reverts a whole crate whose
// fixes error, so beside a shape that errors this one's rewrite would
// be reverted too and the file would come back byte-identical for the
// wrong reason.
//
// Renaming `current_dir` makes the chain head an owned `Command`, and a
// by-value trait method is found before an inherent `&mut self` one, so
// the `.arg` below stops calling `Command::arg` and starts calling
// `Ext::arg`. That compiles, so the fixer has no error to throw the
// file away on.

#![allow(dead_code, unused_imports, reason = "fixture")]

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
