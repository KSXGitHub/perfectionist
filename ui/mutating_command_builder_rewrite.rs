// aux-build:command_extra.rs
// edition:2024
//
// What the automatic rewrite does with a flagged chain, and what stops
// it. The sweep in `ui/mutating_command_builder.rs` is about what the
// diagnostic says; this file is about whether the rewrite is offered
// and how far it reaches.
//
// A `.stderr` cannot tell `MachineApplicable` from `MaybeIncorrect`,
// so a rendered rewrite here means the edit is shown, not that the
// fixer will apply it. What it does show, and what these shapes are
// for, is whether an edit is offered at all and which spans it covers.
// `tests/mutating_command_builder_autofix.rs` runs the real fixer.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::path::Path;
use std::process::{Command, Stdio};

// A borrowed receiver, which several shapes below hand their value to.
fn configure(command: &mut Command) {
    command.arg("-l");
}

// Bad: a macro invocation wrapping the head alone. `$expression:expr`
// keeps the caller's spans, so the chain is rewritten as usual, but its
// span now starts to the left of the flagged call. The `&mut ` goes in
// front of the invocation; written inside it, it would borrow only the
// part the macro was handed, and the tail would then be called on a
// `Command` where `configure` wants the borrow.
macro_rules! passthrough {
    ($expression:expr) => {
        $expression
    };
}

#[expect(
    perfectionist::impure_macro_arguments,
    reason = "the macro has to wrap the chain's head for the prefix's anchor to matter"
)]
fn macro_wrapped_head() {
    configure(passthrough!(Command::new("ls").arg("macro-head")).arg("macro-tail"));
}

// Bad: the argument was written in a macro's body, so the `.into()` the
// counterpart needs would be appended there rather than here, to every
// other expansion of it at once. Advice only, and no rewrite.
mod macro_argument {
    use command_extra::CommandExtra;
    use std::process::{Command, Stdio};

    struct Piped;

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

    fn the_conversion_would_land_in_the_macro() {
        Command::new("ls").stdout(piped!());
    }
}

// Not flagged: an extension trait taking `self` is found at the
// by-value step of the autoderef chain, before `Command`'s own
// `&mut self` setter, so this resolves to `Ext::arg` and renaming it
// would replace a method of the author's own.
mod extension_trait {
    use std::process::Command;

    trait Ext {
        fn arg(self, value: &str) -> Self;
    }

    impl Ext for Command {
        fn arg(self, _value: &str) -> Self {
            self
        }
    }

    fn via_extension_trait() {
        let _: Command = Command::new("ls").arg("ext-trait");
    }
}

// Bad, and no rewrite is rendered: the chain's trailing call is the one
// name the rewrite cannot change, and its receiver turns from a
// `&mut Command` into a `Command`. `Ext::status` takes `self`, so it is
// then found before `Command::status` is reached by autoref. Renaming
// the head alone compiles and calls something else, so the diagnostic
// says what would change rather than showing the line.
mod shadowed_trailing_call {
    use command_extra::CommandExtra;
    use std::io;
    use std::process::{Command, ExitStatus};

    trait Ext {
        fn status(self) -> io::Result<ExitStatus>;
    }

    impl Ext for Command {
        fn status(self) -> io::Result<ExitStatus> {
            Err(io::Error::other("shadowed-trailing-call"))
        }
    }

    fn run() -> io::Result<ExitStatus> {
        Command::new("ls").arg("shadowed-trailing-call").status()
    }
}

// Bad, and no rewrite, for the same reason with a `&self` receiver.
// The `&` autoref is tried before the `&mut` one, so this reaches the
// owned command ahead of `Command::status` just as a `self` receiver
// does -- and a blanket impl means nothing has to be written about
// `Command` at all.
mod shadowed_by_a_borrow {
    use command_extra::CommandExtra;
    use std::io;
    use std::process::{Command, ExitStatus};

    trait Logged {
        fn status(&self) -> io::Result<ExitStatus>;
    }

    impl<Anything: ?Sized> Logged for Anything {
        fn status(&self) -> io::Result<ExitStatus> {
            Err(io::Error::other("shadowed-by-a-borrow"))
        }
    }

    fn run() -> io::Result<ExitStatus> {
        Command::new("ls").arg("shadowed-by-a-borrow").status()
    }
}

// Bad, and the same, for the everyday case of it: `Into::into` takes
// `self` and is in the prelude, so which `From` impl runs is decided by
// the receiver's type.
mod conversion_tail {
    use command_extra::CommandExtra;
    use std::process::Command;

    struct Wrap;

    impl From<&mut Command> for Wrap {
        fn from(_command: &mut Command) -> Wrap {
            Wrap
        }
    }

    impl From<Command> for Wrap {
        fn from(_command: Command) -> Wrap {
            Wrap
        }
    }

    fn run() -> Wrap {
        Command::new("ls").arg("conversion-tail").into()
    }
}

// Bad, and no rewrite either, for the other reason there is: the owned
// command is created after the arguments where the borrow it replaces
// was created before them, so it becomes the statement's last temporary
// and drops first. Only an argument's own destructor is positioned to
// see that.
mod ordered_drop {
    use command_extra::CommandExtra;
    use std::ffi::OsStr;
    use std::process::Command;

    struct Noisy;

    impl Drop for Noisy {
        fn drop(&mut self) {}
    }

    impl AsRef<OsStr> for Noisy {
        fn as_ref(&self) -> &OsStr {
            OsStr::new("ordered-drop")
        }
    }

    struct Holder {
        noisy: Noisy,
        name: String,
    }

    fn holder() -> Holder {
        Holder {
            noisy: Noisy,
            name: String::from("held"),
        }
    }

    fn run() {
        Command::new("ordered-drop").arg(&Noisy);
    }

    // Bad, and no rewrite either: `name` is moved out and the rest of
    // the temporary stays for the statement to drop. Reading a part of
    // a value is not the same as handing the value over, so the
    // argument leaves something behind even though nothing borrowed it.
    fn field_of_a_temporary_argument() {
        Command::new("ordered-drop-field").arg(holder().name);
    }

    // Bad, and no rewrite: a scrutinee is not handed over either. The
    // pattern binds nothing, so the temporary is still there for the
    // statement to drop. Every parent the walk does not recognise as a
    // hand-over answers this way, which is the safe way round.
    fn scrutinee_argument() {
        Command::new("ordered-drop-match").arg(match Noisy {
            _ => "matched",
        });
    }

    struct Tagged {
        noisy: Noisy,
        tag: String,
    }

    fn tagged() -> Tagged {
        Tagged {
            noisy: Noisy,
            tag: String::from("tag"),
        }
    }

    // Bad, and rewritten: the destructor-bearing field is the one moved
    // into the call, so nothing left behind has one. The question is
    // what the rest of the temporary holds, not what the whole of it
    // held.
    fn the_droppable_field_is_the_one_taken() {
        Command::new("ordered-drop-taken").arg(tagged().noisy);
    }

    struct Guard(&'static str);

    impl Drop for Guard {
        fn drop(&mut self) {}
    }

    // Bad, and no rewrite: the destructor is on the base's own type, so
    // it sits on no field and a scan of the siblings finds nothing. A
    // type carrying one cannot be taken apart at all, so reading a
    // field copies or borrows it and the whole of the base stays.
    //
    // The field read here is `Copy`, which is what makes this the shape
    // that tests the question: borrow it instead and the answer comes
    // from the borrow rather than from the base's type.
    fn a_destructor_on_the_base_itself() {
        Command::new("ordered-drop-base").arg(Guard("--base").0);
    }

    struct Inner {
        noisy: Noisy,
        label: &'static str,
    }

    struct Outer {
        inner: Inner,
    }

    fn outer() -> Outer {
        Outer {
            inner: Inner {
                noisy: Noisy,
                label: "nested",
            },
        }
    }

    // Bad, and no rewrite: `inner` is not moved either, only `label` is
    // read out of it, so the question has to be asked at every level a
    // projection goes through rather than at the first.
    fn a_projection_of_a_projection() {
        Command::new("ordered-drop-nested").arg(outer().inner.label);
    }

    // Bad, and rewritten: `?` moves its payload out of the branch it
    // builds, so the argument is handed over and nothing is left to be
    // reordered.
    fn the_question_mark_hands_it_over() -> std::io::Result<()> {
        Command::new("ordered-drop-try").stdin(std::fs::File::open("/dev/null")?);
        Ok(())
    }

    // Bad, and no rewrite: the drop-order line has to survive a written
    // turbofish, since renaming by hand reorders the destructors either
    // way.
    fn a_turbofish_does_not_hide_the_hazard() {
        Command::new("ordered-drop-turbofish").arg::<&str>(&holder().name);
    }

    impl AsRef<OsStr> for Tagged {
        fn as_ref(&self) -> &OsStr {
            OsStr::new("tagged")
        }
    }
}

// Bad, and no rewrite: the command is a field of a temporary whose
// other field has a destructor. Moving the command out leaves that
// field for the statement to drop, and the owned command the change
// produces is created later, so it drops first. `field_of_a_temporary`
// above is the same shape with nothing else to drop.
mod sibling_of_a_drop {
    use command_extra::CommandExtra;
    use std::process::Command;

    struct Noisy;

    impl Drop for Noisy {
        fn drop(&mut self) {}
    }

    struct Bundle {
        noisy: Noisy,
        command: Command,
    }

    fn bundle() -> Bundle {
        Bundle {
            noisy: Noisy,
            command: Command::new("ls"),
        }
    }

    fn run() {
        bundle().command.arg("sibling-of-a-drop");
    }

    fn pair() -> (Noisy, Command) {
        (Noisy, Command::new("ls"))
    }

    // Bad, and no rewrite, for the same shape spelled as a tuple. A
    // tuple carries its field types directly rather than through an
    // `AdtDef`, so asking only about structs answered no here.
    fn tuple_sibling() {
        pair().1.arg("tuple-sibling");
    }

    fn quiet_pair() -> (String, Command) {
        (String::from("quiet"), Command::new("ls"))
    }

    // Bad, and rewritten: the tuple's other element has no destructor
    // to reorder. The question a tuple gets is the same one a struct
    // gets, not a refusal to look.
    fn tuple_without_a_sibling_drop() {
        quiet_pair().1.arg("tuple-quiet");
    }
}

// Bad, and applied: the argument is moved into the call, so nothing is
// left behind whose destructor the change could reorder. Only a value
// something took a reference to outlives the call, which is what
// `ordered_drop` above has and this does not.
fn moved_argument() {
    Command::new("ls").stdin(Stdio::null());
}

// Bad, and no rename is rendered: the chain's value lands in a `let`,
// so the whole rewrite is declined, and renaming the head alone would
// leave `.arg` written against a receiver that rename has just made
// owned, where `Ext::arg` is found before `Command`'s own. A rendered
// rename is a concrete edit, so it is shown only where it moves nothing
// else.
mod partial_rename_would_move_a_call {
    use command_extra::CommandExtra;
    use std::process::Command;

    trait Ext {
        fn arg(self, value: &str) -> &'static str;
    }

    impl Ext for Command {
        fn arg(self, _value: &str) -> &'static str {
            "ext"
        }
    }

    fn run() {
        let _ = Command::new("ls")
            .current_dir("/tmp")
            .arg("partial-rename");
    }
}

// Bad, over a binding, and the diagnostic has to say that `status`
// would move as well. The rewrite is deferred for the receiver, so
// nothing withholds a rename here -- but the reader following the
// advice by hand walks into the same re-resolution, and only this line
// warns them.
mod defers_and_moves {
    use command_extra::CommandExtra;
    use std::io;
    use std::process::{Command, ExitStatus};

    trait Logged {
        fn status(&self) -> io::Result<ExitStatus>;
    }

    impl<Anything: ?Sized> Logged for Anything {
        fn status(&self) -> io::Result<ExitStatus> {
            Err(io::Error::other("defers-and-moves"))
        }
    }

    fn run() {
        let mut command = Command::new("ls");
        let _ = command.arg("defers-and-moves").status();
    }
}


fn main() {}
