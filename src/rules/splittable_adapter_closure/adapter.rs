//! Which adapters the chain trigger speaks about, where each one's item
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

use super::family::Family;
use rustc_span::Symbol;

/// One adapter the chain trigger speaks about.
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

/// An adapter whose closure is its second argument, the first being a
/// default or the other channel's closure.
const fn defaulted(lift_target: &'static str) -> Adapter {
    Adapter {
        closure_argument: 1,
        item_parameter: 0,
        keeps_the_last_step: true,
        lift_target,
    }
}

/// An adapter taking state first, where the whole chain leaves.
const fn stateful(lift_target: &'static str) -> Adapter {
    Adapter {
        closure_argument: 1,
        item_parameter: 1,
        keeps_the_last_step: false,
        lift_target,
    }
}

/// The adapter `method` is on `family`, or `None` for a method the chain
/// trigger does not speak about.
pub(super) fn adapter(family: Family, method: Symbol) -> Option<Adapter> {
    match family {
        Family::Iterator => iterator(method),
        Family::Option => option(method),
        Family::Result => result(method),
        Family::Poll => poll(method),
        Family::ControlFlow => control_flow(method),
        Family::Itertools => itertools(method),
        Family::Rayon => rayon(method),
        Family::Pipe => pipe(method),
    }
}

fn iterator(method: Symbol) -> Option<Adapter> {
    // The guard-and-value trigger answers first for `filter_map`,
    // `find_map` and `map_while`, so a body that is `Option` work gets the
    // split that hands each half to its counterpart. What reaches here is
    // a body that is a chain, which lifts into a leading `map` as any
    // other adapter's does.
    Some(match method.as_str() {
        "map" | "flat_map" | "for_each" | "try_for_each" => unary("map"),
        "filter_map" | "find_map" | "map_while" => unary("map"),
        "any" | "all" | "position" | "rposition" => unary("map"),
        "fold" | "try_fold" | "rfold" | "try_rfold" | "scan" => stateful("map"),
        _ => return None,
    })
}

fn option(method: Symbol) -> Option<Adapter> {
    // `filter`, `inspect` and `take_if` hand the value back, and
    // `or_else`, `ok_or_else`, `unwrap_or_else` and `get_or_insert_with`
    // hand the closure nothing, so neither kind has a chain to read.
    //
    // `is_none_or` is here and not in the conjunction trigger's table,
    // which is not an inconsistency: filtering before it flips what it
    // answers, where a leading `map` leaves every `None` a `None`.
    Some(match method.as_str() {
        "map" | "and_then" | "is_some_and" | "is_none_or" => unary("map"),
        "map_or" | "map_or_else" => defaulted("map"),
        _ => return None,
    })
}

fn result(method: Symbol) -> Option<Adapter> {
    // `inspect` and `inspect_err` hand the value back. `map_or_else` takes
    // one closure per channel, and the table holds one adapter per name,
    // so the value one is read and a chain in the error one is left alone.
    Some(match method.as_str() {
        "map" | "and_then" | "is_ok_and" => unary("map"),
        "map_err" | "or_else" | "unwrap_or_else" | "is_err_and" => unary("map_err"),
        "map_or" | "map_or_else" => defaulted("map"),
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

fn itertools(method: Symbol) -> Option<Adapter> {
    // `tree_reduce` takes the item twice as `Iterator::reduce` does.
    // Excluded for taking the item by reference: `unique_by`,
    // `filter_ok`, `update`, `find_position`, `into_group_map_by`,
    // `position_max_by_key`, `position_min_by_key` and `sorted_by_key`.
    Some(match method.as_str() {
        // The item here is the `Ok` inside a `Result` item, a channel
        // nested one level inside the iterator's own, so it lifts into
        // the adapter mapping that channel. `filter_map_ok`'s closure
        // returns an `Option`, which the guard-and-value trigger has no
        // split for on this channel, so the chain is what is left to read.
        "map_ok" | "filter_map_ok" => unary("map_ok"),
        "fold_ok" => stateful("map_ok"),
        // `partition_map`'s closure returns an `Either`, which is a value
        // it makes rather than the item handed back, so the steps under it
        // lift like any other adapter's.
        "counts_by" | "partition_map" => unary("map"),
        "fold_while" => stateful("map"),
        _ => return None,
    })
}

fn rayon(method: Symbol) -> Option<Adapter> {
    // There is no `scan`, no `map_while` and no `rposition`; `fold`
    // yields per-chunk accumulators rather than one value, and its item
    // side splits all the same. The `position_*` trio is declared on
    // `IndexedParallelIterator`, which is why `extension` looks that up
    // too.
    Some(match method.as_str() {
        "map" | "for_each" | "any" | "all" => unary("map"),
        "position_any" | "position_first" | "position_last" => unary("map"),
        "find_map_any" | "find_map_first" | "find_map_last" => unary("map"),
        "fold" | "try_fold" => stateful("map"),
        _ => return None,
    })
}

fn pipe(method: Symbol) -> Option<Adapter> {
    // Only the by-value form meets the condition. `pipe_ref`,
    // `pipe_mut` and their kin hand the closure a borrow, which a
    // leading `pipe` would be handing something else.
    Some(match method.as_str() {
        "pipe" => unary("pipe"),
        _ => return None,
    })
}
