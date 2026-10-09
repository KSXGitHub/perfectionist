// edition:2024
//
// What a lifted test survives being handed one reference more than it had.
// These adapters take the item by value where the `filter` a test lifts
// into hands a reference, so every mention of the item sits one reference
// deeper once split: a method call autoderefs down to it, an argument
// reaches it only where its parameter is a coercion site, and a trait
// method can be answered by an impl on the reference instead.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::collections::HashSet;

fn checksum(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

trait Marker {
    fn marked(&self) -> bool;
}

struct Concrete;

impl Marker for Concrete {
    fn marked(&self) -> bool {
        true
    }
}

fn marked_by(value: &dyn Marker) -> bool {
    value.marked()
}

fn wants_owned<Subject: Into<String>>(subject: Subject) -> bool {
    subject.into().len() > 1
}

fn even(count: usize) -> bool {
    count % 2 == 0
}

fn large(count: usize) -> bool {
    count > 10
}

struct Counter {
    seen: usize,
}

impl Counter {
    fn bump(&mut self) -> bool {
        self.seen += 1;
        true
    }

    fn flagged(&self) -> bool {
        self.seen > 1
    }
}

// Not flagged: `any` hands the item over where `filter` hands a
// reference, so a test needing it mutably has nothing to get `&mut` out
// of `&&mut`.
fn mutable_item_by_value(counters: &mut [Counter]) -> bool {
    counters.iter_mut().any(|counter| counter.bump() && counter.flagged())
}

// Not flagged: `Option::is_some_and` hands the item over by value, where
// the `filter` a test lifts into hands a reference.
fn mutable_option_item(slot: Option<&mut Counter>) -> bool {
    slot.is_some_and(|counter| counter.bump() && counter.flagged())
}

// Not flagged: an owned item bound `mut` is written to just as a `&mut`
// one is, with no `&mut` in the type to read it from.
fn owned_mutable_item(counters: Vec<Counter>) -> bool {
    counters
        .into_iter()
        .any(|mut counter| counter.bump() && counter.flagged())
}

// Not flagged: an owned item handed to a helper by value gets a
// reference the helper cannot take, since `&T` does not coerce to `T`.
fn owned_item_by_value(mut counts: std::vec::IntoIter<usize>) -> bool {
    counts.any(|count| even(count) && large(count))
}

// Bad: an adapter handed the item by value, where every lifted test reads
// it through a method call, which autoderefs to whatever depth it is
// handed.
fn owned_item_by_method(names: std::vec::IntoIter<String>) -> bool {
    names
        .into_iter()
        .any(|name| name.starts_with('a') && name.ends_with('z'))
}

// Good: the leading test filters, its method call autoderefing through the
// reference the `filter` hands it.
fn split_owned_item_by_method(names: std::vec::IntoIter<String>) -> bool {
    names
        .filter(|name| name.starts_with('a'))
        .any(|name| name.ends_with('z'))
}

// Bad: a `Copy` item read through a method that takes `self`, which the
// autoderef can produce a value for.
fn copy_item_by_self(mut letters: std::vec::IntoIter<char>) -> bool {
    letters.any(|letter| letter.is_alphabetic() && letter.is_uppercase())
}

// Good: the leading test filters, the `Copy` item being read through the
// reference without being moved out of it.
fn split_copy_item_by_self(letters: std::vec::IntoIter<char>) -> bool {
    letters
        .filter(|letter| letter.is_alphabetic())
        .any(|letter| letter.is_uppercase())
}

// Not flagged: a trait method can be intercepted one reference up, where
// `IntoIterator for &[T; N]` yields a reference to each byte rather than
// the byte.
fn trait_method_on_the_receiver(rows: Vec<[u8; 4]>) -> bool {
    rows.into_iter()
        .any(|row| row.into_iter().any(|byte| byte == 0) && row.first().is_some())
}

// Not flagged: `Clone for &T` is the same interception, handing back the
// reference where the item's own `clone` hands back a value.
fn clone_on_the_receiver(words: Vec<String>) -> bool {
    words
        .iter()
        .any(|word| word.clone().into_bytes().is_empty() && word.is_empty())
}

// Not flagged: an unsize coercion at the outer reference has no step to
// repeat, `&[u8; 32]` reaching `&[u8]` where `&&[u8; 32]` does not.
fn unsized_parameter(hashes: Vec<[u8; 32]>) -> bool {
    hashes.iter().any(|hash| checksum(hash) && hash.is_ascii())
}

// Not flagged: a `dyn` coercion at the outer reference has no step to
// repeat, `&Concrete` reaching `&dyn Marker` where `&&Concrete` does not,
// the user trait carrying no blanket impl for a reference.
fn dyn_parameter(items: Vec<Concrete>) -> bool {
    items.iter().any(|item| marked_by(item) && item.marked())
}

// Not flagged: a reference parameter naming a type parameter is no
// coercion site either, the extra reference being inferred into the
// parameter instead of coerced away.
fn generic_reference_parameter(words: Vec<String>, allowed: &HashSet<String>) -> bool {
    words
        .iter()
        .any(|word| allowed.contains(word) && !word.is_empty())
}

// Not flagged: a parameter naming a type parameter is not a coercion
// site, so the extra reference is inferred into the parameter and the
// bound is checked against it rather than against what it was written for.
fn generic_parameter(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| wants_owned(line) && !line.is_empty())
}

fn main() {}
