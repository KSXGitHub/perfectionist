// aux-build:rayon_stub.rs
// edition:2024
//
// rayon's parallel adapters, which the rule identifies by path because the
// trait carries no diagnostic item. Two things separate them from the
// sequential ones: a lifted step's result has to cross a thread, so it must
// be `Send`, and the methods come in `_any` / `_first` / `_last` flavours
// that differ in which item's answer wins rather than in what the closure
// is handed.
//
// The crate is not in the driver's sysroot, so a stub stands in for it,
// carrying only the methods these cases call.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate rayon;

use rayon::prelude::*;
use std::rc::Rc;

fn wanted(text: &str) -> bool {
    !text.is_empty()
}

// Bad: a parallel adapter, which splits the way the sequential one
// does.
fn parallel(items: Parallel<&'static str>) -> Parallel<usize> {
    items.map(|text| text.trim().len())
}

// Good: the same split.
fn split_parallel(items: Parallel<&'static str>) -> Parallel<usize> {
    items.map(str::trim).map(str::len)
}

// Bad: a parallel predicate-returning consumer.
fn parallel_any(items: Parallel<&'static str>) -> bool {
    items.any(|text| text.trim().is_empty())
}

// Good: the step lifted, leaving the consumer its test.
fn split_parallel_any(items: Parallel<&'static str>) -> bool {
    items.map(str::trim).any(str::is_empty)
}

// Bad: a parallel predicate that is a conjunction, which the conjunction
// trigger splits into successive `filter`s.
fn parallel_conjunction(items: Parallel<&'static str>) -> bool {
    items.any(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading one filtering.
fn split_parallel_conjunction(items: Parallel<&'static str>) -> bool {
    items
        .filter(|text| wanted(text))
        .any(|text| text.starts_with('#'))
}

// Not flagged: a parallel `filter` hands the item back downstream, the
// way the sequential one does.
fn parallel_filter(items: Parallel<&'static str>) -> Parallel<&'static str> {
    items.filter(|text| text.trim().is_empty())
}

// Not flagged: a lifted step's result has to cross a thread, and an `Rc`
// cannot. The folded form compiles because the value never leaves the
// closure.
fn not_sendable(items: Parallel<&'static str>) -> Parallel<usize> {
    items.map(|text| Rc::new(text).len())
}

// Not flagged: the guard-and-value trigger reads `Iterator`'s own
// `filter_map` alone, and what each combinator hands its work to was tabled
// for `Iterator`, so rayon's is left to the chain trigger.
fn parallel_guarded(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .filter_map(|text| wanted(text).then(|| text.len()))
        .into_items()
}

// Bad: rayon's `for_each` consumes in parallel, and its item enters by
// value and never comes back out.
fn parallel_each(items: Parallel<&'static str>) {
    items.for_each(|text| drop(text.trim().len()));
}

// Good: one adapter per step, `for_each` left the consuming.
fn split_parallel_each(items: Parallel<&'static str>) {
    items.map(str::trim).map(str::len).for_each(drop);
}

// Bad: rayon's `all` asks one test of every item, in whatever order the
// pool reaches them.
fn parallel_every(items: Parallel<&'static str>) -> bool {
    items.all(|text| text.trim().is_empty())
}

// Good: one adapter per step, `all` left the test.
fn split_parallel_every(items: Parallel<&'static str>) -> bool {
    items.map(str::trim).all(str::is_empty)
}

// Bad: `position_any` answers with the index of whichever passing item the
// pool reaches first, and a leading `map` keeps one item per item, so the
// indices are the ones they were.
fn parallel_position(items: Parallel<&'static str>) -> Option<usize> {
    items.position_any(|text| text.trim().is_empty())
}

// Good: one adapter per step, `position_any` left the answering.
fn split_parallel_position(items: Parallel<&'static str>) -> Option<usize> {
    items.map(str::trim).position_any(str::is_empty)
}

// Bad: `position_first` pins the answer to the earliest passing item
// rather than the earliest reached, the indices still one per item.
fn parallel_position_first(items: Parallel<&'static str>) -> Option<usize> {
    items.position_first(|text| text.trim().is_empty())
}

// Good: one adapter per step, `position_first` left the answering.
fn split_parallel_position_first(items: Parallel<&'static str>) -> Option<usize> {
    items.map(str::trim).position_first(str::is_empty)
}

// Bad: `position_last` pins the answer to the latest passing item, and
// counts the same items the folded form counted.
fn parallel_position_last(items: Parallel<&'static str>) -> Option<usize> {
    items.position_last(|text| text.trim().is_empty())
}

// Good: one adapter per step, `position_last` left the answering.
fn split_parallel_position_last(items: Parallel<&'static str>) -> Option<usize> {
    items.map(str::trim).position_last(str::is_empty)
}

// Bad: `find_map_any` takes whichever answer the pool produces first, and
// its item enters by value.
fn parallel_found_any(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_any(|text| text.trim().parse().ok())
}

// Good: one adapter per step, `find_map_any` left the discarding of errors.
fn split_parallel_found_any(items: Parallel<&'static str>) -> Option<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map_any(Result::ok)
}

// Bad: `find_map_first` takes the answer belonging to the earliest item
// rather than the earliest produced.
fn parallel_found_first(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_first(|text| text.trim().parse().ok())
}

// Good: one adapter per step, `find_map_first` left the discarding of
// errors.
fn split_parallel_found_first(items: Parallel<&'static str>) -> Option<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map_first(Result::ok)
}

// Bad: `find_map_last` takes the answer belonging to the latest item.
fn parallel_found_last(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_last(|text| text.trim().parse().ok())
}

// Good: one adapter per step, `find_map_last` left the discarding of
// errors.
fn split_parallel_found_last(items: Parallel<&'static str>) -> Option<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map_last(Result::ok)
}

// Bad: rayon's `fold` takes an accumulator per thread, so it is handed a
// closure whose *second* parameter is the item.
fn parallel_total(items: Parallel<&'static str>) -> Parallel<usize> {
    items.fold(|| 0, |total, text| total + text.trim().len())
}

// Good: one adapter per step, `fold` left the accumulation alone.
fn split_parallel_total(items: Parallel<&'static str>) -> Parallel<usize> {
    items
        .map(str::trim)
        .map(str::len)
        .fold(|| 0, |total, length| total + length)
}

// Bad: rayon's `try_fold` can stop early, and takes the item as its
// closure's second parameter, the per-thread accumulator being the first.
fn parallel_try_total(items: Parallel<&'static str>) -> Parallel<Option<usize>> {
    items.try_fold(|| 0, |total, text| Some(total + text.trim().len()))
}

// Good: one adapter per step, `try_fold` left the accumulation alone.
fn split_parallel_try_total(items: Parallel<&'static str>) -> Parallel<Option<usize>> {
    items
        .map(str::trim)
        .map(str::len)
        .try_fold(|| 0, |total, length| Some(total + length))
}

// Bad: `find_first` hands the closure a borrow of the item, which a leading
// `map` would replace, so the chain trigger leaves it alone and the
// conjunction trigger's `filter`, taking the item the same way, reports.
fn parallel_first(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find_first(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading `filter` handing the item the way
// `find_first` did.
fn split_parallel_first(items: Parallel<&'static str>) -> Option<&'static str> {
    items
        .filter(|text| wanted(text))
        .find_first(|text| text.starts_with('#'))
}

// Bad: `find_any` hands the closure a borrow of the item, so the conjunction
// trigger is the one with a split for it.
fn parallel_found(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find_any(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading `filter` handing the item the way
// `find_any` did.
fn split_parallel_found(items: Parallel<&'static str>) -> Option<&'static str> {
    items
        .filter(|text| wanted(text))
        .find_any(|text| text.starts_with('#'))
}

fn main() {}
