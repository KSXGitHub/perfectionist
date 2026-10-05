// aux-build:orx_parallel_4_1_1.rs
// edition:2024
//
// `orx-parallel`'s trait as 4.0 declares it, where which adapter is which is
// read from the signature each was declared with rather than from a table of
// names. The trait is named `Par` here and `ParIter` one major earlier, and
// the companion fixture holds the rule to that one.
//
// Every Bad case is followed by the Good one the rule's help asks for.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate orx_parallel;

use orx_parallel::{Par, Parallel};

fn record(length: usize) {}

fn wanted(text: &str) -> bool {
    !text.is_empty()
}

// Bad: `map` is handed the item, which is what a leading `map` of its own
// can be given instead.
fn mapping(items: Parallel<&'static str>) -> Vec<usize> {
    items.map(|text| text.trim().len()).into_items()
}

// Good: one adapter per step.
fn split_mapping(items: Parallel<&'static str>) -> Vec<usize> {
    items.map(str::trim).map(str::len).into_items()
}

// Bad: `for_each` is handed the item too, consuming rather than adapting.
fn each(items: Parallel<&'static str>) {
    items.for_each(|text| record(text.trim().len()));
}

// Good: the steps lifted, `for_each` left the consuming.
fn split_each(items: Parallel<&'static str>) {
    items.map(str::trim).map(str::len).for_each(record);
}

// Bad: `fold` takes state first, so the item is its closure's second
// parameter and the whole chain leaves.
fn total(items: Parallel<&'static str>) -> Vec<usize> {
    items.fold(|| 0, |sum, text| *sum += text.trim().len())
}

// Good: the steps lifted, `fold` left the accumulation alone.
fn split_total(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .map(str::trim)
        .map(str::len)
        .fold(|| 0, |sum, length| *sum += length)
}

// Bad: `any` is lent the item rather than handed it, so the chain stays and
// the conjunction is what splits.
fn either(items: Parallel<&'static str>) -> bool {
    items.any(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading `filter` lent the item the way
// `any` was.
fn split_either(items: Parallel<&'static str>) -> bool {
    items
        .filter(|text| wanted(text))
        .any(|text| text.starts_with('#'))
}

// Bad: `find` is lent the item for the same kind of reason, and the
// conjunction has the split.
fn found(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading `filter` lent the item the way
// `find` was.
fn split_found(items: Parallel<&'static str>) -> Option<&'static str> {
    items
        .filter(|text| wanted(text))
        .find(|text| text.starts_with('#'))
}

// Not flagged: `inspect` is lent the item and hands it back, so a leading
// `map` would change what it and every later adapter sees.
fn inspecting(items: Parallel<&'static str>) -> Vec<&'static str> {
    items.inspect(|text| record(text.trim().len())).into_items()
}

// Not flagged: filtering before `all` makes an item that failed the first
// test vacuously fine.
fn every(items: Parallel<&'static str>) -> bool {
    items.all(|text| wanted(text) && text.starts_with('#'))
}

// Not flagged: `reduce` is handed two items rather than state and an item,
// so no leading `map` holds what it accumulates.
fn reducing(items: Parallel<&'static str>) -> Option<&'static str> {
    items.reduce(|left, right| match left.len() > right.len() {
        true => left,
        false => right,
    })
}

fn main() {}
