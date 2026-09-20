// Shapes `mutating_command_builder` declines to hand the fixer, for
// `tests/mutating_command_builder_autofix.rs`; `applied.rs` holds the
// ones it rewrites. The test asserts this file comes back
// byte-identical, and `silent_redirect` is what makes that assertion
// able to fail: its rewrite would compile, so `cargo fix` would have no
// error to revert on and an applied rewrite would survive on disk.

#![allow(dead_code, unused_imports, reason = "fixture")]

// The value is a method receiver, where `&mut` would need parentheses
// and read worse than the call it replaces. That the bare rename is
// also unsound here is the reason it is only ever shown: renaming
// `current_dir` makes the chain head owned, and a by-value trait method
// is found before an inherent `&mut self` one, so `.arg` below would
// stop calling `Command::arg` and start calling `Ext::arg`, compiling
// all the while.
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

// The receiver is a binding the next statement reads, so the rewrite
// would need an edit at a span the rule was not given.
pub mod over_a_binding {
    use command_extra::CommandExtra;
    use std::process::Command;

    pub fn run() {
        let mut command = Command::new("ls");
        command.arg("over-a-binding");
        let _ = command.status();
    }
}

// A written turbofish names the setter's generic parameters, and the
// counterpart's do not correspond to them.
pub mod turbofish {
    use command_extra::CommandExtra;
    use std::process::Command;

    pub fn run() {
        Command::new("ls").args::<[&str; 1], &str>(["turbofish"]);
    }
}

// The counterpart cannot be written here until the `use` arrives.
pub mod trait_out_of_scope {
    use std::process::Command;

    pub fn run() {
        Command::new("ls").arg("trait-out-of-scope");
    }
}

// The argument builds a value whose destructor runs at a point the
// rewrite would move: the owned command is created after the argument,
// so it would drop before `Noisy` rather than after it.
pub mod ordered_drop {
    use command_extra::CommandExtra;
    use std::ffi::OsStr;
    use std::process::Command;

    pub struct Noisy;

    impl Drop for Noisy {
        fn drop(&mut self) {}
    }

    impl AsRef<OsStr> for Noisy {
        fn as_ref(&self) -> &OsStr {
            OsStr::new("ordered-drop")
        }
    }

    pub fn run() {
        Command::new("ordered-drop").arg(&Noisy);
    }
}
