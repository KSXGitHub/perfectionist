// edition:2024
//
// `itertools`' own adapters, which the rule identifies by path because the
// trait carries no diagnostic item. Its nested-channel methods are the
// shape to know: `map_ok` and its kin adapt the `Ok` inside a `Result`
// item, so the chain they hold is one level in from the iterator's own.
//
// The crate needs no stub. The driver's sysroot ships the real one, because
// rustc itself depends on it, and a stub of that name collides with it
// (`E0464`); reaching it from there is what `rustc_private` is for.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool, rustc_private)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate itertools;

use itertools::Itertools;
use std::collections::HashMap;

fn classify(text: &'static str) -> itertools::Either<usize, &'static str> {
    match text.len() > 3 {
        true => itertools::Either::Left(text.len()),
        false => itertools::Either::Right(text),
    }
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

// Good: the steps lifted into the adapter mapping that channel, leaving
// the closure the accumulation.
fn split_nested_fold(items: std::vec::IntoIter<Result<&'static str, usize>>) -> Result<usize, usize> {
    items
        .map_ok(str::trim)
        .map_ok(str::len)
        .fold_ok(0, |total, length| total + length)
}

// Bad: binary over the iterator's own item, so this one lifts into `map`.
fn own_fold(mut items: std::vec::IntoIter<&'static str>) -> usize {
    items
        .fold_while(0, |total, text| {
            itertools::FoldWhile::Continue(total + text.trim().len())
        })
        .into_inner()
}

// Good: the steps lifted into `map`, this adapter's item being the
// iterator's own.
fn split_own_fold(items: std::vec::IntoIter<&'static str>) -> usize {
    items
        .map(str::trim)
        .map(str::len)
        .fold_while(0, |total, length| itertools::FoldWhile::Continue(total + length))
        .into_inner()
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

// Good: every step but the last lifted into `map_ok`, leaving the adapter
// the call that discards the error.
fn split_nested_filter_map(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items
        .map_ok(str::trim)
        .map_ok(str::parse::<usize>)
        .filter_map_ok(Result::ok)
        .collect()
}

// Not flagged: one step is already one adapter doing one thing.
fn one_step(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<&'static str, usize>> {
    items.map_ok(|text| text.trim()).collect()
}

// Bad: `counts_by` keys a tally by what its closure answers, and the item
// enters it by value.
fn counting(lines: std::vec::IntoIter<&'static str>) -> HashMap<usize, usize> {
    lines.counts_by(|text| text.trim().len())
}

// Good: one adapter per step, `counts_by` left the tallying.
fn split_counting(lines: std::vec::IntoIter<&'static str>) -> HashMap<usize, usize> {
    lines.map(str::trim).counts_by(str::len)
}

// Bad: `partition_map` sends each item one way or the other by what its
// closure answers, the item entering by value.
fn partitioned(lines: std::vec::IntoIter<&'static str>) -> (Vec<usize>, Vec<&'static str>) {
    lines.partition_map(|text| classify(text.trim()))
}

// Good: one adapter per step, `partition_map` left the sorting into two.
fn split_partitioned(
    lines: std::vec::IntoIter<&'static str>,
) -> (Vec<usize>, Vec<&'static str>) {
    lines.map(str::trim).partition_map(classify)
}

// Not flagged: `unique_by` is lent the item rather than handed it, so a
// leading `map` would change what it and every later adapter sees. It is one
// of the `itertools` adapters the table leaves out for that reason.
fn unique(items: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    items.unique_by(|text| text.trim().len()).collect()
}

fn main() {}
