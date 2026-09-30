// aux-build:command_extra.rs
// edition:2024
//
// UI sweep for `folded_command_setter`. A fold over a singular
// `CommandExtra` setter is flagged and offered the plural; the folder is
// matched by what it resolves to, so every spelling of the same setter
// behaves alike. What the receiver looks like decides how much of it the
// suggestion keeps, and whether the fix is applied at all.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate command_extra;

use command_extra::CommandExtra;
use std::ffi::OsStr;
use std::process::Command;

const VARS: &[&str] = &["A", "B"];
const PAIRS: &[(&str, &str)] = &[("A", "1"), ("B", "2")];

// Bad: the three pairs, spelled as a path. The receiver's `iter` comes
// from the standard library, so the fix is applied -- but the call
// survives into it, because only `into_iter` is erased.
fn paths() {
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::without_env);
    let _ = VARS.iter().fold(Command::new("ls"), CommandExtra::with_arg);
}

// Bad: the same setter reached through the concrete type, through a
// fully-qualified path, and through a renamed import — one method, so
// one shape.
use command_extra::CommandExtra as Ext;

fn spellings() {
    let _ = VARS.iter().fold(Command::new("ls"), Command::without_env);
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), <Command as CommandExtra>::without_env);
    let _ = VARS.iter().fold(Command::new("ls"), Ext::without_env);
}

// Bad: a closure that forwards its parameters, written as a method call
// and as an associated-function call.
fn closures() {
    let _ = VARS.iter().fold(Command::new("ls"), |c, v| c.without_env(v));
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, v| CommandExtra::without_env(c, v));
}

// Bad: the `with_env` pair, whose item is a pair. No path spelling
// works — `with_env` takes three arguments where `fold` supplies two —
// so the closure destructures and forwards the bindings in order.
fn pair_item(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: the same pair over a map reference, whose items are pairs of
// references. The `iter` is std's, so the fix is applied.
fn pair_item_borrowed(pairs: &std::collections::HashMap<String, String>) {
    let _ = pairs
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Not flagged: a reference to a pair is not a pair. `with_envs` takes
// `IntoIterator<Item = (Key, Value)>`, and `&[(&str, &str)]` yields
// `&(&str, &str)`, which the fold binds through and the plural cannot.
fn reference_to_a_pair() {
    let _ = PAIRS
        .iter()
        .fold(Command::new("ls"), |command, (key, value)| {
            command.with_env(key, value)
        });
}

// Bad: a bare place expression as the receiver. Nothing to erase, and
// nothing to evaluate, so the fix is applied.
fn bare_receiver(names: std::slice::Iter<'_, &'static str>) {
    let _ = names.fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `into_iter` resolving to the trait method the plural calls
// itself, so the call is erased.
fn owned_into_iter(names: Vec<String>) {
    let _ = names
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `iter` on an owned collection, which the fold only borrows. The
// call survives, as every `iter` does.
fn owned_iter(names: Vec<String>) {
    let _ = names.iter().fold(Command::new("ls"), CommandExtra::without_env);
    let _ = names.len();
}

// Bad: a field reached through `self`, which is a place expression.
struct Config {
    names: Vec<String>,
}

impl Config {
    fn build(&self) -> Command {
        self.names
            .iter()
            .fold(Command::new("ls"), CommandExtra::without_env)
    }
}

// Bad: an `iter` of the linted crate's own. Flagged, but the fix is
// withheld: the call is not the standard library's, so what evaluating
// it does is unknown.
struct Weird(Vec<String>);

impl Weird {
    fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

fn local_iter(weird: Weird) {
    let _ = weird
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: an inherent `into_iter` shadowing the trait in method
// resolution. Flagged, call kept, fix withheld.
struct Shadow(Vec<String>);

impl Shadow {
    fn into_iter(self) -> std::vec::IntoIter<String> {
        self.0.into_iter()
    }
}

fn shadowing_into_iter(shadow: Shadow) {
    let _ = shadow
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: an argument-less call that mutates. Condition 4 admits it, so the
// rule fires; the fix is withheld because the initial value could
// observe the mutation in the new order.
struct Queue(Vec<String>);

impl Queue {
    fn drain_all(&mut self) -> std::vec::Drain<'_, String> {
        self.0.drain(..)
    }

    fn take_all(&mut self) -> std::vec::IntoIter<String> {
        std::mem::take(&mut self.0).into_iter()
    }
}

fn mutating_receiver(mut queue: Queue) {
    let _ = queue
        .drain_all()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: the same mutating receiver against an initial value that reads
// what it changes. Advice only, and this one stays advice however far
// the gate is widened: the two orders build different commands.
fn mutating_receiver_observed(mut queue: Queue) {
    let _ = queue.take_all().fold(
        Command::new(format!("ls{}", queue.0.len())),
        CommandExtra::without_env,
    );
}

// Not flagged: two calls on the receiver. The second is more text
// moving, so the rewrite would relocate rather than remove.
fn two_calls(names: Vec<String>) {
    let _ = names
        .into_iter()
        .rev()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: a call taking an argument, which is where logic hides.
fn call_with_argument(names: Vec<String>) {
    let _ = names
        .iter()
        .map(String::as_str)
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: the same shape written as an associated-function call on
// the receiver. A known syntactic gap, not a regression.
fn assoc_fn_receiver(names: Vec<String>) {
    let _ = Vec::into_iter(names).fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: a closure that computes on the way. The plural would not
// remove it — it would move one call to the left and add a `map`.
fn transforming_closure() {
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, v| c.with_arg(format!("--{v}")));
}

// Not flagged: a closure whose block holds a statement, which is where
// a closure computes. A block that only wraps the forwarding call reads
// alike and is flagged — the `with_env` pairs above are written that
// way.
fn block_body() {
    let _ = VARS.iter().fold(Command::new("ls"), |command, var| {
        let key = var;
        command.without_env(key)
    });
}

// Not flagged: arguments passed out of the order they were bound.
fn swapped_arguments() {
    let _ = PAIRS
        .iter()
        .fold(Command::new("ls"), |c, (key, value)| c.with_env(value, key));
}

// Not flagged: a singular with no plural. Each sets one thing a later
// call replaces rather than extends.
fn no_plural() {
    let _ = VARS
        .iter()
        .fold(Command::new("ls"), |c, _| c.with_no_env());
}

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
fn std_setter() {
    let _ = VARS.iter().fold(Command::new("ls"), |mut command, var| {
        command.arg(var);
        command
    });
}

// Bad: a receiver the call did not run on. Method resolution derefs
// `&&Vec<String>` twice to reach `<[T]>::iter`, so the place is not the
// receiver, and handing the plural `v` would hand it something that is
// not an iterator at all. The call survives, so it compiles.
fn double_reference(names: &&Vec<String>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: the same through a smart pointer.
fn through_rc(names: &std::rc::Rc<Vec<String>>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `into_iter` reached by an autoref, because the only
// `IntoIterator` is on the reference. Erasing would leave `Borrowed`,
// which is not an iterator, so the adjustment check keeps the call.
struct Borrowed(Vec<String>);

impl<'a> IntoIterator for &'a Borrowed {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

fn autoref_into_iter(names: Borrowed) {
    let _ = names
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: `iter` is std's by way of `Deref` while the type keeps an
// `IntoIterator` of its own that yields the other order. Erasing here
// would compile and silently reverse the arguments, which is why no
// `iter` is erased.
struct Backwards(Vec<String>);

impl std::ops::Deref for Backwards {
    type Target = Vec<String>;

    fn deref(&self) -> &Vec<String> {
        &self.0
    }
}

impl<'a> IntoIterator for &'a Backwards {
    type Item = &'a String;
    type IntoIter = std::iter::Rev<std::slice::Iter<'a, String>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().rev()
    }
}

fn disagreeing_into_iter(names: &Backwards) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Bad: a `&mut` place, which erasing would move where the fold only
// borrowed.
fn mutable_reference(names: &mut Vec<String>) {
    let _ = names
        .iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
    let _ = names.len();
}

// Bad: an initial value that binds looser than a method call, so the
// suggestion has to bracket it.
fn looser_initial(command: Box<Command>) {
    let _ = VARS.iter().fold(*command, CommandExtra::without_env);
}

// Not flagged: a fold inside a `macro_rules!` of this crate's own. The
// suggestion would replace the definition with text read from a call
// site, and two invocations would earn two suggestions at one span.
macro_rules! scrub {
    ($start:expr) => {
        VARS.iter().fold($start, CommandExtra::without_env)
    };
}

fn in_a_macro_body() {
    let start = Command::new("ls");
    let _ = scrub!(start);
}

// Not flagged: an overloaded `Deref` in the place. Reaching the place
// runs the `deref` body, and the suggestion moves that across the
// initial value.
struct Counted(Vec<String>);

struct Noisy(Counted);

impl std::ops::Deref for Noisy {
    type Target = Counted;

    fn deref(&self) -> &Counted {
        DEREFS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        &self.0
    }
}

impl<'a> IntoIterator for &'a Counted {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

static DEREFS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn overloaded_deref(noisy: Noisy) {
    let _ = (*noisy)
        .into_iter()
        .fold(Command::new("ls"), CommandExtra::without_env);
}

// Not flagged: a block holding a statement that is not a rebinding, so
// the `uses` check would accept the call while the statement is where
// the closure computes. The plural would delete it.
fn block_with_an_effect() {
    let _ = VARS.iter().fold(Command::new("ls"), |command, var| {
        println!("{var}");
        command.without_env(var)
    });
}

// Not flagged: a `..` in the tuple pattern hides a field, so the
// bindings the closure forwards are not all of the item.
fn gapped_tuple(triples: Vec<(String, String, String)>) {
    let _ = triples
        .into_iter()
        .fold(Command::new("ls"), |command, (key, ..)| {
            command.without_env(key)
        });
}

// Not flagged: fewer arguments than the item has bindings. The plural
// would pass the whole pair where the fold passed one half of it.
fn dropped_binding(pairs: Vec<(String, String)>) {
    let _ = pairs
        .into_iter()
        .fold(Command::new("ls"), |command, (key, _value)| {
            command.without_env(key)
        });
}

fn main() {}
