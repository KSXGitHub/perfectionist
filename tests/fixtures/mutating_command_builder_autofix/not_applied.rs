// Shapes `mutating_command_builder` declines to hand the fixer, for
// `tests/mutating_command_builder_autofix.rs`; `applied.rs` holds the
// ones it rewrites.
//
// What that test asserts is again the rule's decision rather than the
// compiler's, and it says why a file that comes back unchanged proves
// anything. The exceptions to its argument are `trait_out_of_scope`,
// which names a method that does not exist yet, and `macro_argument`,
// whose rewrite has no type to infer: neither would compile if it were
// rewritten.
//
// `shadowed_trailing_call` and `ordered_drop` are the ones that would
// compile *and* be wrong, so nothing but the rule's own decision is
// positioned to withhold them.

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

// The argument builds a value whose destructor the rewrite would
// reorder, as `ui/mutating_command_builder_rewrite.rs`'s `ordered_drop`
// shows.
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

// The chain's trailing call would resolve to `Ext::status` once its
// receiver became owned, as
// `ui/mutating_command_builder_rewrite.rs`'s `shadowed_trailing_call`
// shows. The rewrite would compile and call something else.
pub mod shadowed_trailing_call {
    use command_extra::CommandExtra;
    use std::io;
    use std::process::{Command, ExitStatus};

    pub trait Ext {
        fn status(self) -> io::Result<ExitStatus>;
    }

    impl Ext for Command {
        fn status(self) -> io::Result<ExitStatus> {
            Err(io::Error::other("shadowed-trailing-call"))
        }
    }

    pub fn run() -> io::Result<ExitStatus> {
        Command::new("ls").arg("shadowed-trailing-call").status()
    }
}
