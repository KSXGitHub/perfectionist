// Shapes `mutating_command_builder` declines to hand the fixer, for
// `tests/mutating_command_builder_autofix.rs`; `applied.rs` holds the
// ones it rewrites.
//
// What that test asserts is again the rule's decision rather than the
// compiler's: it requires this file back byte-identical, and requires
// the rule to have fired on every shape in it, so silence cannot pass
// for restraint. Some of these would compile if they were rewritten
// anyway -- `trait_out_of_scope` names a method that does not exist
// yet, and `macro_argument`'s rewrite has no type to infer -- so a file
// that comes back unchanged did so because nothing was offered, not
// because `cargo fix` reverted it.

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

// The argument was written in a macro's body, so the `.into()` the
// counterpart needs would be appended there rather than here.
pub mod macro_argument {
    use command_extra::CommandExtra;
    use std::process::{Command, Stdio};

    pub struct Piped;

    impl From<Piped> for Stdio {
        fn from(_piped: Piped) -> Stdio {
            Stdio::null()
        }
    }

    macro_rules! piped {
        () => {
            Piped
        };
    }

    pub fn run() {
        Command::new("macro-argument").stdout(piped!());
    }
}

