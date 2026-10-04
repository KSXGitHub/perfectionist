// aux-build:rayon_stub.rs
// aux-build:pipe_trait_stub.rs
// edition:2024
//
// The extension traits, whose paths are what identify them: none carries
// a diagnostic item.
//
// `itertools` needs no stub: the driver's sysroot ships the real crate,
// because rustc itself depends on it, and a stub of that name collides
// with it (`E0464`). Reaching it from there is what `rustc_private` is
// for. rayon and `pipe-trait` are not in the sysroot, so each has a
// stub.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

#![feature(register_tool, rustc_private)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate itertools;
extern crate pipe_trait;
extern crate rayon;

use itertools::Itertools;
use pipe_trait::Pipe;
use rayon::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;

fn record(length: usize) {}

fn double(value: usize) -> usize {
    value * 2
}

fn render(value: usize) -> String {
    value.to_string()
}

fn wanted(text: &str) -> bool {
    !text.is_empty()
}

fn classify(text: &'static str) -> itertools::Either<usize, &'static str> {
    match text.len() > 3 {
        true => itertools::Either::Left(text.len()),
        false => itertools::Either::Right(text),
    }
}

fn results() -> std::vec::IntoIter<Result<&'static str, usize>> {
    vec![Ok(" a "), Err(7)].into_iter()
}

fn lines() -> std::vec::IntoIter<&'static str> {
    vec![" a ", " bb "].into_iter()
}

// Bad: the `Ok` inside a `Result` item is a channel nested one level
// inside the iterator's own, so a lifted step goes into `map_ok`.
fn nested_channel(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items.map_ok(|text| text.trim().len()).collect()
}

// Good: one adapter per step, on the nested channel.
fn split_nested_channel(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items.map_ok(str::trim).map_ok(str::len).collect()
}

// Bad: binary over that same nested item.
fn nested_fold(
    mut items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Result<usize, usize> {
    items.fold_ok(0, |total, text| total + text.trim().len())
}

// Bad: binary over the iterator's own item, so this one lifts into `map`.
fn own_fold(mut items: std::vec::IntoIter<&'static str>) -> usize {
    items
        .fold_while(0, |total, text| {
            itertools::FoldWhile::Continue(total + text.trim().len())
        })
        .into_inner()
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

// Bad: a piping method, where every step becomes one and none stays.
fn piped(value: usize) -> String {
    value.pipe(|number| render(double(number)))
}

// Bad: a parallel predicate that is a conjunction, which the predicate
// rule splits into successive `filter`s.
fn parallel_conjunction(items: Parallel<&'static str>) -> bool {
    items.any(|text| wanted(text) && text.starts_with('#'))
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

// Not flagged: `pipe_ref` hands the closure a borrow, so a leading `pipe`
// would be handing it something else.
fn piped_by_reference(value: String) -> usize {
    value.pipe_ref(|text| text.trim().len())
}

// Bad: `filter_map_ok`'s closure returns an `Option`, and the
// guard-and-value trigger has no split for that on the `Ok` channel, so
// the chain is what is left to read.
fn nested_filter_map(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items
        .filter_map_ok(|text| text.trim().parse().ok())
        .collect()
}

// Not flagged: one step is already one adapter doing one thing.
fn one_step(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<&'static str, usize>> {
    items.map_ok(|text| text.trim()).collect()
}

// Bad: the rest of the extension tables, one case each, so an entry
// naming the wrong parameter or the wrong shape would show up here.
fn counting(lines: std::vec::IntoIter<&'static str>) -> HashMap<usize, usize> {
    lines.counts_by(|text| text.trim().len())
}

fn partitioned(lines: std::vec::IntoIter<&'static str>) -> (Vec<usize>, Vec<&'static str>) {
    lines.partition_map(|text| classify(text.trim()))
}

fn parallel_each(items: Parallel<&'static str>) {
    items.for_each(|text| drop(text.trim().len()));
}

fn parallel_every(items: Parallel<&'static str>) -> bool {
    items.all(|text| text.trim().is_empty())
}

fn parallel_position(items: Parallel<&'static str>) -> Option<usize> {
    items.position_any(|text| text.trim().is_empty())
}

fn parallel_position_first(items: Parallel<&'static str>) -> Option<usize> {
    items.position_first(|text| text.trim().is_empty())
}

fn parallel_position_last(items: Parallel<&'static str>) -> Option<usize> {
    items.position_last(|text| text.trim().is_empty())
}

fn parallel_found_any(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_any(|text| text.trim().parse().ok())
}

fn parallel_found_first(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_first(|text| text.trim().parse().ok())
}

fn parallel_found_last(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_last(|text| text.trim().parse().ok())
}

fn parallel_total(items: Parallel<&'static str>) -> Parallel<usize> {
    items.fold(|| 0, |total, text| total + text.trim().len())
}

fn parallel_try_total(items: Parallel<&'static str>) -> Parallel<Option<usize>> {
    items.try_fold(|| 0, |total, text| Some(total + text.trim().len()))
}

// Not flagged by the chain rule: `find_first` and `find_any` hand the
// closure a borrow of the item, which a leading `map` would replace. Bad
// for the predicate rule, whose `filter` takes the item the same way.
fn parallel_first(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find_first(|text| wanted(text) && text.starts_with('#'))
}

fn parallel_found(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find_any(|text| wanted(text) && text.starts_with('#'))
}

fn main() {}
