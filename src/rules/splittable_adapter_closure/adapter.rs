//! Which adapters this rule speaks about, where each one's item
//! parameter sits in the closure it takes, and what a lifted step goes
//! into.
//!
//! The condition an adapter has to meet is that **the item enters the
//! closure by value and never comes back out**. That is what makes a
//! leading lift mean the same thing: an adapter borrowing the item, or
//! handing it back downstream, answers differently once the lift has
//! replaced what it sees. On iterators `filter`, `find`, `take_while`,
//! `skip_while`, `max_by_key`, `partition` and `reduce` all fail it one
//! way or the other, and `inspect` fails it by passing the item on
//! unchanged; `Option::filter`, `Option::inspect`, `Result::inspect` and
//! `Result::inspect_err` fail it the same way.
//!
//! The table is a list rather than a type query because the condition is
//! about an adapter's contract, which no type exposes. Each entry below
//! was checked against that contract by hand.

use rustc_span::Symbol;

/// Which family a receiver belongs to, which is what tells
/// `Iterator::map` from `Option::map`.
///
/// Each family beyond `Iterator` carries one value rather than a stream,
/// and only its mapping adapters are in scope. A fallible, defaulted or
/// predicate-shaped adapter leaves the lifted step's result wrapped in
/// the one the adapter keeps, which costs the reader a wrapper and
/// sometimes a `mut` rebinding that the folded form did not have. The
/// planning file measured those splits as equivalent, which they are;
/// what it did not weigh is what they read like.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    /// `Iterator` or `DoubleEndedIterator`.
    Iterator,
    /// `Option`.
    Option,
    /// `Result`.
    Result,
    /// `Poll`.
    Poll,
    /// `ControlFlow`.
    ControlFlow,
}

/// One adapter this rule speaks about.
#[derive(Clone, Copy)]
pub(super) struct Adapter {
    /// Which of the method's arguments is the closure holding the item.
    /// A method taking one closure per channel is keyed by this, since
    /// its name alone does not say which channel a closure is on.
    pub(super) closure_argument: usize,
    /// Which of the closure's parameters carries the item. State comes
    /// first where there is any, so a `fold`-shaped closure answers 1.
    pub(super) item_parameter: usize,
    /// Whether the chain's last step stays with the adapter rather than
    /// lifting out. It keeps the adapter holding a closure, which is
    /// what the pipeline's kind depends on. An adapter taking state
    /// keeps that expression instead, so the whole chain leaves.
    pub(super) keeps_the_last_step: bool,
    /// The adapter a lifted step goes into, which is the one mapping
    /// this channel and doing nothing else.
    pub(super) lift_target: &'static str,
}

/// A unary adapter on the only channel its family has.
const fn unary(lift_target: &'static str) -> Adapter {
    Adapter {
        closure_argument: 0,
        item_parameter: 0,
        keeps_the_last_step: true,
        lift_target,
    }
}

/// An adapter taking state first, where the whole chain leaves.
const fn stateful() -> Adapter {
    Adapter {
        closure_argument: 1,
        item_parameter: 1,
        keeps_the_last_step: false,
        lift_target: "map",
    }
}

/// The adapter `method` is on `family`, or `None` for a method this rule
/// does not speak about.
pub(super) fn adapter(family: Family, method: Symbol) -> Option<Adapter> {
    match family {
        Family::Iterator => iterator(method),
        Family::Option => option(method),
        Family::Result => result(method),
        Family::Poll => poll(method),
        Family::ControlFlow => control_flow(method),
    }
}

fn iterator(method: Symbol) -> Option<Adapter> {
    // `filter_map`, `find_map` and `map_while` meet the condition and
    // are left out all the same: their closure returns an `Option`, and
    // what splits inside one is `Option` work, which
    // `perfectionist::splittable_adapter_option_chain` is about.
    Some(match method.as_str() {
        "map" | "flat_map" | "for_each" | "try_for_each" => unary("map"),
        "any" | "all" | "position" | "rposition" => unary("map"),
        "fold" | "try_fold" | "rfold" | "try_rfold" | "scan" => stateful(),
        _ => return None,
    })
}

fn option(method: Symbol) -> Option<Adapter> {
    Some(match method.as_str() {
        "map" => unary("map"),
        _ => return None,
    })
}

fn result(method: Symbol) -> Option<Adapter> {
    Some(match method.as_str() {
        "map" => unary("map"),
        "map_err" => unary("map_err"),
        _ => return None,
    })
}

fn poll(method: Symbol) -> Option<Adapter> {
    Some(match method.as_str() {
        "map" => unary("map"),
        "map_ok" => unary("map_ok"),
        "map_err" => unary("map_err"),
        _ => return None,
    })
}

fn control_flow(method: Symbol) -> Option<Adapter> {
    Some(match method.as_str() {
        "map_break" => unary("map_break"),
        "map_continue" => unary("map_continue"),
        _ => return None,
    })
}
