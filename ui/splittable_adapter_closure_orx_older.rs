// aux-build:orx_parallel_3_3_0.rs
// edition:2024
//
// `orx-parallel`'s trait as 3.x declared it, under the name `ParIter` that
// 4.0 renamed to `Par`, declaring `take_while` and `map_while` and no
// `fold`.
//
// Nothing here is a second table of names. Which adapter is which is read
// from the signature each was declared with, and those did not change across
// the rename, so the same reading answers this generation and the current
// one alike.
//
// What a signature cannot carry is the order the adapter runs in, which is
// why the `take_while` declared only in this generation is read as no
// filtering discipline at all rather than as a prefix-shaped one.
//
// Every Bad case is followed by the Good one the rule's help asks for.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate orx_parallel;

use orx_parallel::{Parallel, ParIter};

fn wanted(text: &str) -> bool {
    !text.is_empty()
}

// Bad: the trait under its older name, `ParIter`, declares `map` handing
// the item over by value, which is what the rule reads rather than the
// name.
fn mapping(items: Parallel<&'static str>) -> Vec<usize> {
    items.map(|text| text.trim().len()).into_items()
}

// Good: one adapter per step.
fn split_mapping(items: Parallel<&'static str>) -> Vec<usize> {
    items.map(str::trim).map(str::len).into_items()
}

// Bad: `map_while` is handed the item and returns an `Option`, which this
// generation declares and the current one does not.
fn parsing(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .map_while(|text| text.trim().parse::<usize>().ok())
        .into_items()
}

// Good: one adapter per step, `map_while` left the item it stops on.
fn split_parsing(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .map_while(Result::ok)
        .into_items()
}

// Not flagged: this `take_while` stops a run, and upstream's docs call the
// result not always deterministic under parallel execution, so the run it
// stops is not established to be the same run twice. Splitting the
// conjunction would make two truncation points out of one, and the
// signature the adapter is declared with says nothing either way.
fn prefix(items: Parallel<&'static str>) -> Vec<&'static str> {
    items
        .take_while(|text| wanted(text) && text.starts_with('#'))
        .into_items()
}

// Bad: `any` is lent the item here as it is under the newer name, so the
// conjunction is what splits.
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

fn main() {}
