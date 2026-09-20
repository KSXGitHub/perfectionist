// Every shape `mutating_command_builder` fires on, for
// `tests/mutating_command_builder_autofix.rs`. Each call carries a
// distinct argument so an assertion there can name one shape without
// matching another.

#![allow(dead_code, unused_imports, unused_mut, reason = "fixture")]

mod trait_in_scope {
    use command_extra::CommandExtra;
    use std::process::Command;

    // Fixable: the rename is the whole fix here.
    pub fn feeds_a_receiver() {
        let _ = Command::new("ls").arg("receiver-in-scope").status();
    }

    // Not fixable: a bare rename moves `command`, which the next
    // statement still reads.
    pub fn statement_position() {
        let mut command = Command::new("ls");
        command.arg("statement-in-scope");
        let _ = command.status();
    }
}

mod chain_on_a_local {
    use command_extra::CommandExtra;
    use std::process::Command;

    // The rule's headline shape: a chain over a `mut` binding the
    // following code still reads.
    pub fn lister() -> Command {
        let mut command = Command::new("ls");
        command.current_dir("/chain-on-a-local").arg("chain-tail");
        command
    }
}

mod captured_by_a_closure {
    use command_extra::CommandExtra;
    use std::process::Command;

    pub fn run() {
        let mut command = Command::new("ls");
        let mut go = || { let _ = command.arg("captured").status(); };
        go();
        go();
    }
}

mod import_inside_a_body {
    use std::process::Command;

    // The module's only import of the trait is body-local, so it
    // does not bring it into scope for the sibling below.
    pub fn imports_it_locally() {
        use command_extra::CommandExtra;
        let _ = Command::new("ls").with_arg("body-local");
    }

    pub fn sibling() {
        let _ = Command::new("ls").arg("body-local-sibling").status();
    }
}

mod other_applicability_gates {
    use command_extra::CommandExtra;
    use std::process::Command;

    // A blanket impl instantiates `Self` to the receiver's type, so
    // it sees `&mut Command` now and `Command` after a rename.
    pub trait Piped { fn piped<R>(self, f: impl FnOnce(Self) -> R) -> R where Self: Sized { f(self) } }
    impl<T> Piped for T {}

    pub fn blanket_impl_parent() {
        Command::new("ls").arg("blanket-parent").piped(|c: &mut Command| { let _ = c.status(); });
    }

    // A turbofish survives into a method of different generic arity.
    pub fn turbofish() {
        let _ = Command::new("ls").args::<[&str; 1], &str>(["turbofish"]).status();
    }

    // The value is a function argument, not a method receiver, so
    // the changed type is what the context rejects.
    pub fn not_a_receiver() {
        configure(Command::new("ls").arg("not-a-receiver"));
    }
    fn configure(_c: &mut Command) {}

    // A rename inside a macro body is written to the definition.
    macro_rules! add { ($c:expr) => { $c.arg("macro-body") }; }
    pub fn from_a_macro() {
        let _ = add!(Command::new("ls")).status();
    }
}

// Not fixable: this module has no `use command_extra::CommandExtra`,
// so a rename would be `no method named with_arg found`. The crate
// is loaded -- the module above imports it -- so the dependency gate
// passes and the diagnostic still fires.
mod trait_out_of_scope {
    use std::process::Command;

    pub fn feeds_a_receiver() {
        let _ = Command::new("ls").arg("receiver-out-of-scope").status();
    }
}
