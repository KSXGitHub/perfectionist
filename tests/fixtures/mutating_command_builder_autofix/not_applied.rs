// Shapes `mutating_command_builder` declines to hand the fixer, for
// `tests/mutating_command_builder_autofix.rs`; `applied.rs` holds the
// ones it rewrites. The test asserts this file comes back
// byte-identical.
//
// Three of the four would compile if they were rewritten anyway, so the
// comparison can fail rather than passing because `cargo fix` reverted
// the file: only `trait_out_of_scope` names a method that does not
// exist yet.

#![allow(dead_code, unused_imports, reason = "fixture")]

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
