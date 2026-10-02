// Shapes `folded_command_setter` fires on and declines to hand the
// fixer, for `tests/folded_command_setter_autofix.rs`. The test compares
// this file against itself, so anything the fixer rewrote fails it.
//
// Each fold carries a distinct name, its program, a binding or a const,
// so an assertion can name one shape without matching another.

#![allow(dead_code, unused_imports, reason = "fixture")]

use command_extra::CommandExtra;
use std::cell::Cell;
use std::ffi::OsStr;
use std::pin::Pin;
use std::process::Command;
use std::sync::LazyLock;

pub struct Weird(Vec<String>);

impl Weird {
    pub fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

// An `iter` of the linted crate's own.
pub fn local_iter(weird: &Weird) -> Command {
    weird.iter().fold(Command::new("local-iter"), CommandExtra::without_env)
}

pub struct Shadow(Vec<String>);

impl Shadow {
    pub fn into_iter(self) -> std::vec::IntoIter<String> {
        self.0.into_iter()
    }
}

// An inherent `into_iter` shadowing the trait in method resolution.
pub fn shadowing_into_iter(shadow: Shadow) -> Command {
    shadow.into_iter().fold(Command::new("shadowed-into-iter"), CommandExtra::without_env)
}

pub struct Queue(Vec<String>);

impl Queue {
    pub fn take_all(&mut self) -> std::vec::IntoIter<String> {
        std::mem::take(&mut self.0).into_iter()
    }
}

// An argument-less call that mutates what the initial value never
// reads: safe to rewrite, but the call is the user's.
pub fn mutating_receiver(queue: &mut Queue) -> Command {
    queue.take_all().fold(Command::new("mutating-receiver"), CommandExtra::without_env)
}

// `iter` reached through a `Deref` the user wrote.
pub struct Backwards(Vec<String>);

impl std::ops::Deref for Backwards {
    type Target = Vec<String>;

    fn deref(&self) -> &Vec<String> {
        &self.0
    }
}

pub fn user_deref(names: &Backwards) -> Command {
    names.iter().fold(Command::new("user-deref"), CommandExtra::without_env)
}

// A user's `Deref` written as `*`.
pub fn explicit_deref(names: Backwards) -> Command {
    (*names).iter().fold(Command::new("explicit-deref"), CommandExtra::without_env)
}

#[derive(Clone, Copy)]
pub struct Countdown(u8);

impl Iterator for Countdown {
    type Item = &'static str;

    fn next(&mut self) -> Option<&'static str> {
        let remaining = self.0.checked_sub(1)?;
        self.0 = remaining;
        Some("A")
    }
}

fn exhaust(countdown: &mut Countdown, program: &str) -> Command {
    countdown.0 = 0;
    Command::new(program)
}

// A place the initial value writes. The fold copies `countdown` before
// `exhaust` empties it, and the plural would copy it afterwards.
pub fn written_place(mut countdown: Countdown) -> Command {
    countdown.fold(exhaust(&mut countdown, "written-place"), CommandExtra::without_env)
}

// A `static mut`, which anything can write, a call the initial value
// makes included.
static mut STATIC_COUNTDOWN: Countdown = Countdown(1);

pub fn static_mut_place() -> Command {
    unsafe { STATIC_COUNTDOWN.fold(Command::new("static-mut-place"), CommandExtra::without_env) }
}

// A place the initial value assigns to directly.
pub fn assigned_place(mut countdown: Countdown) -> Command {
    countdown.fold({ countdown.0 = 0; Command::new("assigned-place") }, CommandExtra::without_env)
}

// A captured place the initial value writes inside a closure.
pub fn written_in_closure(mut countdown: Countdown) -> impl FnMut() -> Command {
    move || countdown.fold(exhaust(&mut countdown, "written-in-closure"), CommandExtra::without_env)
}

// A copy read through a reference the body took of a local, which the
// initial value then borrows mutably. The fold copies `*view` first, and
// the plural would borrow first, which is `E0502`.
pub fn aliased_copy(mut countdown: Countdown) -> Command {
    let view = &countdown;
    view.fold(exhaust(&mut countdown, "aliased-copy"), CommandExtra::without_env)
}

// A reference to a local the initial value borrows mutably, whose
// `'static` field is what the receiver keeps.
pub struct Config {
    flags: [&'static str; 2],
}

fn clear_flags(config: &mut Config, program: &str) -> Command {
    config.flags = ["", ""];
    Command::new(program)
}

pub fn aliased_static_items(mut config: Config) -> Command {
    let view = &config;
    view.flags.into_iter().fold(clear_flags(&mut config, "aliased-static-items"), CommandExtra::with_arg)
}

// A copy read through a raw pointer, which can point anywhere.
pub unsafe fn raw_copy(countdown: *const Countdown) -> Command {
    unsafe { (*countdown).fold(Command::new("raw-copy"), CommandExtra::without_env) }
}

// A `Deref` the user wrote on the way to a field, rather than on the
// place the call runs on.
pub struct Names {
    names: Vec<String>,
}

pub struct Handle(Names);

impl std::ops::Deref for Handle {
    type Target = Names;

    fn deref(&self) -> &Names {
        &self.0
    }
}

pub fn deref_under_field(handle: Handle) -> Command {
    handle.names.iter().fold(Command::new("deref-under-field"), CommandExtra::without_env)
}

// A `Deref` the standard library wrote that calls the user's: `Pin`'s
// calls `Backwards`'s.
pub fn pinned_deref(names: Pin<Backwards>) -> Command {
    names.iter().fold(Command::new("pinned-deref"), CommandExtra::without_env)
}

// A `LazyLock`, whose first `Deref` runs the user's initializer, and
// whose state a shared reference can write.
static LAZY: LazyLock<Vec<String>> = LazyLock::new(|| vec!["A".to_owned()]);

pub fn lazy_lock() -> Command {
    LAZY.iter().fold(Command::new("lazy-lock"), CommandExtra::without_env)
}

fn reset(cell: &Cell<std::vec::IntoIter<String>>, program: &str) -> Command {
    cell.set(Vec::new().into_iter());
    Command::new(program)
}

// A write through a shared reference, which leaves `cell` itself
// unmutated as far as the borrow checker is concerned.
pub fn interior_mutable(cell: Cell<std::vec::IntoIter<String>>) -> Command {
    cell.take().fold(reset(&cell, "interior-mutable"), CommandExtra::without_env)
}

// A std trait's method whose body is the user's: `Borrowed`'s
// `into_iter`.
pub struct Borrowed(Vec<String>);

impl<'a> IntoIterator for &'a Borrowed {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub fn user_into_iter(borrowed: Borrowed) -> Command {
    borrowed.into_iter().fold(Command::new("user-into-iter"), CommandExtra::without_env)
}

// A std method that runs the user's code: cloning a `vec::IntoIter`
// clones each item, and `Item`'s `clone` is the user's.
pub struct Item(String);

impl Clone for Item {
    fn clone(&self) -> Self {
        Item(self.0.clone())
    }
}

impl AsRef<OsStr> for Item {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

pub fn user_item_clone(items: std::vec::IntoIter<Item>) -> Command {
    items.clone().fold(Command::new("user-item-clone"), CommandExtra::without_env)
}

// A std call that panics on its caller's behalf. `unwrap` panics before
// the initial value runs in the fold, and after it in the plural.
pub fn panicking_receiver(names: Option<std::vec::IntoIter<String>>) -> Command {
    names.unwrap().fold(Command::new("panicking-receiver"), CommandExtra::without_env)
}

pub struct Job {
    removed: &'static [&'static str],
}

impl Job {
    fn into_base(self, program: &str) -> Command {
        std::mem::drop(self);
        Command::new(program)
    }
}

// An initial value that consumes the place the receiver is rooted at.
// The fold reads `job.removed` before `job` is moved, and the plural
// would read it after, which is `E0382`.
pub fn moved_root(job: Job) -> Command {
    job.removed.iter().fold(job.into_base("moved-root"), CommandExtra::without_env)
}

// An initial value whose type the folder supplies: `into` converts into
// a `Command` because `Command::without_env` takes one. As the plural's
// receiver it would convert into nothing in particular, which is
// `E0282`.
pub fn inferred_initial(names: &[&str]) -> Command {
    names.iter().fold(Command::new("inferred-initial").into(), Command::without_env)
}

// `into` through a binding that names no type.
pub fn inferred_binding(names: &[&str]) -> Command {
    let inferred_binding = Command::new("ls").into();
    names.iter().fold(inferred_binding, Command::without_env)
}

// Bindings that name no type: a closure parameter, a binding inside a
// `let` pattern, and a `match` arm's.
pub fn inferred_closure_parameter(names: &'static [&'static str]) -> impl Fn(Command) -> Command {
    |inferred_closure_parameter| names.iter().fold(inferred_closure_parameter, Command::without_env)
}

pub fn inferred_let_pattern(names: &[&str]) -> Command {
    let (inferred_let_pattern, _) = (Command::new("ls").into(), 0);
    names.iter().fold(inferred_let_pattern, Command::without_env)
}

pub fn inferred_match_arm(names: &[&str]) -> Command {
    match Command::new("ls").into() {
        inferred_match_arm => names.iter().fold(inferred_match_arm, Command::without_env),
    }
}

// A branch that diverges, which fixes no type, beside one whose type the
// fold supplies.
pub fn diverging_branch(names: &[&str], flag: bool, diverging_branch: Command) -> Command {
    names.iter().fold(if flag { diverging_branch.into() } else { todo!() }, Command::without_env)
}

// A closure folder whose annotation is all that fixes the item type.
// The suggestion drops the closure, and the plural over an element type
// nothing else names is `E0282`.
pub fn item_annotated() -> Command {
    let names = Vec::new();
    names.iter().fold(Command::new("item-annotated"), |command, name: &&str| command.with_arg(name))
}

// The item type fixed only by a turbofish on `fold`, which the
// suggestion drops.
pub fn turbofish() -> Command {
    let names = Vec::new();
    names.into_iter().fold::<Command, fn(Command, &'static str) -> Command>(Command::new("turbofish"), CommandExtra::with_arg)
}

// A comment outside the initial value and the receiver, which the
// suggestion has no place for.
pub fn dropped_comment(names: Vec<String>) -> Command {
    names
        .into_iter()
        // kept only by hand
        .fold(Command::new("dropped-comment"), CommandExtra::without_env)
}

// An initial value that is the receiver's root, moved into the fold
// after the receiver has borrowed through it. The plural would move it
// first, which is `E0382`.
pub struct Defaults {
    defaults: &'static [&'static str],
}

impl CommandExtra for Defaults {
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

pub fn initial_root(initial_is_the_root: Defaults) -> Defaults {
    initial_is_the_root.defaults.iter().fold(initial_is_the_root, CommandExtra::with_arg)
}

const OVERRIDDEN_VARS: &[&str] = &["a"];
const AMBIGUOUS_VARS: &[&str] = &["a"];
// A type whose impl writes its own body for the plural. The override
// keeps the trait's signature, so the rewrite compiles, but it would run
// that body in place of the fold.
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
    OVERRIDDEN_VARS.iter().fold(overriding, CommandExtra::without_env)
}

// A type that another trait also gives a method named like the plural,
// so wherever both traits are in scope the rewrite is `E0034`.
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
    AMBIGUOUS_VARS.iter().fold(ambiguous, CommandExtra::without_env)
}

// The plural is named rather than resolved, and this module does not
// import the trait. A *path* folder needs no import of its own, so the
// fold compiles where the rewrite would be `E0599`.
pub mod trait_not_in_scope {
    use std::process::Command;

    const NOT_IN_SCOPE: &[&str] = &["a"];

    pub fn run(command: Command) -> Command {
        NOT_IN_SCOPE
            .iter()
            .fold(command, command_extra::CommandExtra::without_env)
    }
}

// Another crate compiled as `command_extra` is imported, and its
// `CommandExtra` has no plural. The trait the fold names is not in scope,
// and the rewrite would be `E0599`.
pub mod other_trait_in_scope {
    use fake::CommandExtra;
    use std::process::Command;

    const OTHER_TRAIT: &[&str] = &["a"];

    pub fn run(command: Command) -> Command {
        OTHER_TRAIT
            .iter()
            .fold(command, command_extra::CommandExtra::without_env)
    }
}
