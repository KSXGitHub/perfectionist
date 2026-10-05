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

use mine::Par as _;
use orx_parallel::mapless::{self, Par as _};
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

// Not flagged: `reduce` is handed the item twice, so a step lifted out of
// the first parameter would be applied to the second as well. The first
// parameter is mentioned once here, which is what reaches the gate; reading
// only that parameter would make `reduce` look like `fold`, whose first
// parameter is an accumulator no lift touches.
fn reducing(items: Parallel<&'static str>) -> Option<&'static str> {
    items.reduce(|left, right| match left.trim().is_empty() {
        true => right,
        false => right,
    })
}

// Not flagged: `max_by` is handed the item twice as well, and behind a
// reference, so neither trigger has a split for it.
fn largest(items: Parallel<&'static str>) -> Option<&'static str> {
    items.max_by(|left, right| left.len().cmp(&right.len()))
}

// Bad: `filter_map` is handed the item and answers an `Option`.
fn parsing(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .filter_map(|text| text.trim().parse::<usize>().ok())
        .into_items()
}

// Good: one adapter per step, `filter_map` left the question of the
// `Option`.
fn split_parsing(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .filter_map(Result::ok)
        .into_items()
}

// Bad: `flat_map` is handed the item too, and flattens what it answers.
fn letters(items: Parallel<&'static str>) -> Vec<char> {
    items.flat_map(|text| text.trim().chars()).into_items()
}

// Good: one adapter per step, `flat_map` left the flattening.
fn split_letters(items: Parallel<&'static str>) -> Vec<char> {
    items.map(str::trim).flat_map(str::chars).into_items()
}

// Bad: the item is lent to `any`, so binding it mutably rebinds the
// reference and a lifted test still reads the same item.
fn either_rebound(items: Parallel<&'static str>) -> bool {
    items.any(|mut text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the rebinding left where it is read.
fn split_either_rebound(items: Parallel<&'static str>) -> bool {
    items
        .filter(|text| wanted(text))
        .any(|mut text| text.starts_with('#'))
}

// Not flagged: a conjunction inside a `map` is the value the adapter
// answers, not a test it runs, so a leading `filter` would drop items
// `map` has to keep.
fn mapped_conjunction(items: Parallel<&'static str>) -> Vec<bool> {
    items
        .map(|text| wanted(text) && text.starts_with('#'))
        .into_items()
}

// Not flagged: this trait declares no `map`, so there is no adapter for a
// lifted step to go into.
fn without_a_lift_target(items: mapless::Parallel<&'static str>) {
    items.for_each(|text| record(text.trim().len()));
}

// Not flagged: a `Par` of the author's own is not `orx-parallel`'s, so its
// signatures are not read the way that crate's are.
fn own_trait(items: mine::Parallel<&'static str>) -> Vec<usize> {
    items.map(|text| text.trim().len()).into_items()
}

/// A trait of the author's own that happens to carry the name.
mod mine {
    pub struct Parallel<Item>(pub Vec<Item>);

    pub trait Par: Sized {
        type Item;

        fn into_items(self) -> Vec<Self::Item>;

        fn map<Out, H>(self, h: H) -> Parallel<Out>
        where
            H: Fn(Self::Item) -> Out,
        {
            Parallel(self.into_items().into_iter().map(h).collect())
        }
    }

    impl<Item> Par for Parallel<Item> {
        type Item = Item;

        fn into_items(self) -> Vec<Item> {
            self.0
        }
    }
}

fn main() {}
