// aux-build:command_extra.rs
// edition:2024
//
// What the rule does and does not ask of the fold's accumulator. It
// never asks the accumulator's type: the folder resolving to a
// `CommandExtra` setter is what proves the accumulator implements the
// trait.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];

// Not flagged: an accumulator that is not a `Command`.
fn other_accumulator() {
    let _ = VARS.iter().fold(String::new(), |mut joined, var| {
        joined.push_str(var);
        joined
    });
}

// Not flagged: a fold over a std setter. The folder resolves to
// `Command::arg` rather than a `CommandExtra` setter, and its block
// holds a statement, so this is the sibling rule's to speak about.
#[expect(
    perfectionist::mutating_command_builder,
    reason = "the sibling rule owning this line is the point of the case"
)]
fn std_setter() {
    let _ = VARS.iter().fold(Command::new("ls"), |mut command, var| {
        command.arg(var);
        command
    });
}

// Bad: an accumulator the code names only by a type parameter. What
// proves the accumulator is a `CommandExtra` is the folder resolving to
// one of its setters, so the rule never asks the accumulator's type --
// which is also how it reaches whatever else a release implements the
// trait for. The fix is withheld, because the parameter can stand for a
// type whose impl writes its own plural.
fn generic_accumulator<Builder: CommandExtra>(command: Builder) -> Builder {
    VARS.iter().fold(command, CommandExtra::without_env)
}

fn opaque() -> impl CommandExtra {
    Command::new("ls")
}

// Bad: an accumulator of an opaque type, which stands for a type the
// same way a parameter does. The fix is withheld.
fn opaque_accumulator() -> impl CommandExtra {
    VARS.iter().fold(opaque(), CommandExtra::without_env)
}

// Not flagged: an accumulator that never arrives. The fold does not run,
// so the plural would strip an environment the fold left alone.
fn diverging_initial(flag: bool) -> Command {
    if flag {
        return VARS.iter().fold(return Command::new("ls"), CommandExtra::without_env);
    }
    Command::new("ls")
}

// Not flagged: the accumulator's own type carries an inherent method of
// the plural's name, which method resolution reaches before the trait's.
struct Shadowing(Command);

impl Shadowing {
    fn without_envs(self, _count: usize) -> Self {
        self
    }
}

impl CommandExtra for Shadowing {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

fn inherent_shadow(command: Shadowing) -> Shadowing {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Bad: an inherent method of the plural's name that takes `&self`.
// Method resolution finds the by-value plural first, so the rewritten
// call reaches the trait's.
struct Borrowing(Command);

impl Borrowing {
    fn without_envs(&self, _count: usize) -> usize {
        0
    }
}

impl CommandExtra for Borrowing {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

fn inherent_by_reference(command: Borrowing) -> Borrowing {
    VARS.iter().fold(command, CommandExtra::without_env)
}

// Bad: an inherent method of the plural's name on another instantiation
// of the accumulator's type, which method resolution never reaches for
// this one.
struct Wrapper<Inner>(Inner);

impl Wrapper<u32> {
    fn without_envs(self, _count: usize) -> Self {
        self
    }
}

impl<Inner> CommandExtra for Wrapper<Inner> {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

fn other_instantiation(wrapped: Wrapper<String>) -> Wrapper<String> {
    VARS.iter().fold(wrapped, CommandExtra::without_env)
}

// Not flagged: an inherent method of the plural's name on the type a
// `Box` holds, taking the box by value. Method resolution reaches it
// before the trait's plural for the box.
struct Boxed;

impl Boxed {
    fn without_envs(self: Box<Self>, _count: usize) -> usize {
        0
    }
}

impl CommandExtra for Box<Boxed> {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

fn boxed_receiver(boxed: Box<Boxed>) -> Box<Boxed> {
    VARS.iter().fold(boxed, CommandExtra::without_env)
}

// Bad: a type whose impl writes its own body for the plural. The
// override keeps the trait's signature, so the rewrite compiles, but it
// would run that body in place of the fold, and the fix is withheld.
pub struct Overriding;

impl CommandExtra for Overriding {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn without_envs<Keys>(self, _keys: Keys) -> Self
    where
        Keys: IntoIterator,
        Keys::Item: AsRef<OsStr>,
    {
        self
    }
}

pub fn overriding(overriding: Overriding) -> Overriding {
    VARS.iter().fold(overriding, CommandExtra::without_env)
}

// Bad: a type that another trait also gives a method named like the
// plural, so wherever both traits are in scope the rewrite is `E0034`.
// The fix is withheld.
pub struct Ambiguous;

impl CommandExtra for Ambiguous {
    fn with_current_dir(self, _dir: impl AsRef<std::path::Path>) -> Self {
        self
    }
    fn with_env(self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> Self {
        self
    }
    fn without_env(self, _key: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_no_env(self) -> Self {
        self
    }
    fn with_arg(self, _arg: impl AsRef<OsStr>) -> Self {
        self
    }
    fn with_stdin(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stdout(self, _stdio: std::process::Stdio) -> Self {
        self
    }
    fn with_stderr(self, _stdio: std::process::Stdio) -> Self {
        self
    }
}

pub trait SameName {
    fn without_envs<Keys: IntoIterator>(self, keys: Keys) -> Self;
}

impl SameName for Ambiguous {
    fn without_envs<Keys: IntoIterator>(self, _keys: Keys) -> Self {
        self
    }
}

pub fn ambiguous(ambiguous: Ambiguous) -> Ambiguous {
    VARS.iter().fold(ambiguous, CommandExtra::without_env)
}

// Bad, and the fix is applied: another trait declares a method named
// like the plural, but on `&self`. Method probing reaches the by-value
// plural first, so the two never compete.
trait ByReference {
    fn without_envs<Keys: IntoIterator>(&self, keys: Keys) -> usize;
}

impl ByReference for Command {
    fn without_envs<Keys: IntoIterator>(&self, keys: Keys) -> usize {
        keys.into_iter().count()
    }
}

fn by_reference(command: Command) -> Command {
    VARS.iter().fold(command, CommandExtra::without_env)
}

fn main() {}
