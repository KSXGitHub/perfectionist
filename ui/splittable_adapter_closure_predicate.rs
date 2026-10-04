// edition:2024
//
// Which predicates the rule splits, and which it leaves alone.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

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

fn longer(line: &str, limit: usize) -> bool {
    line.len() > limit
}

fn shorter(line: &str, limit: usize) -> bool {
    line.len() < limit * 2
}

fn even(count: usize) -> bool {
    count % 2 == 0
}

fn large(count: usize) -> bool {
    count > 10
}

fn wanted(line: &str) -> bool {
    !line.is_empty()
}

// Bad: two tests on the item.
fn two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Good: one adapter per test, which is what the rule asks for.
fn split_two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#'))
        .collect()
}

// Bad: three tests, so the count in the message is not always two.
fn three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && line.starts_with('#') && line.ends_with('!'))
        .collect()
}

// Good: one adapter per test, however many there are.
fn split_three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#'))
        .filter(|line| line.ends_with('!'))
        .collect()
}

// Bad: a disjunction inside a conjunct stays whole, and the pair still
// splits.
fn conjunct_holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && (line.starts_with('#') || line.ends_with('!')))
        .collect()
}

// Good: the disjunction stays one test, in an adapter of its own.
fn split_conjunct_holds_a_disjunction(
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#') || line.ends_with('!'))
        .collect()
}

// Bad: a consumer whose answer still depends only on which items
// satisfy the predicate.
fn find_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.find(|line| wanted(line) && line.starts_with('#'))
}

// Good: the leading test filters, leaving the consumer the last one.
fn split_find_tests(lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines
        .filter(|line| wanted(line))
        .find(|line| line.starts_with('#'))
}

// Bad: the same from the other end.
fn rfind_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.rfind(|line| wanted(line) && line.starts_with('#'))
}

// Good: the same from the other end, `Filter` being double-ended wherever
// its iterator is.
fn split_rfind_tests(lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines
        .filter(|line| wanted(line))
        .rfind(|line| line.starts_with('#'))
}

// Bad: `any` takes its item by value where the lifted `filter` hands a
// reference, and a `&&str` derefs to where these tests read it.
fn any_tests(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| wanted(line) && line.starts_with('#'))
}

// Good: the leading test filters, which hands it one reference more than
// `any` did, and a `&&str` derefs to where the test reads it.
fn split_any_tests(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines
        .filter(|line| wanted(line))
        .any(|line| line.starts_with('#'))
}

// Bad: a prefix-shaped adapter, whose tests lift into a `take_while`
// rather than a `filter`.
fn take_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .take_while(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Good: one `take_while` per test, which stops the run on the first item
// failing either, as the conjunction did.
fn split_take_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .take_while(|line| wanted(line))
        .take_while(|line| line.starts_with('#'))
        .collect()
}

// Not flagged: one test is already one adapter asking one question.
fn one_test(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| wanted(line)).collect()
}

// Not flagged: a disjunction keeps items the pair would have dropped.
fn disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) || line.starts_with('#'))
        .collect()
}

// Not flagged: filtering before `all` makes an item that failed the
// first test vacuously fine, so the answer flips.
fn all_tests(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.all(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: filtering renumbers what `position` counts.
fn position_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.position(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: neither target reproduces what `skip_while` drops.
fn skip_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .skip_while(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Not flagged: a conjunct naming no item holds for every item or none,
// so it wants hoisting out of the pipeline rather than an adapter.
fn invariant_conjunct(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| flag && wanted(line)).collect()
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

// Not flagged: and an owned item bound `mut` is written to just as a
// `&mut` one is, with no `&mut` in the type to read it from.
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

// Bad: the same adapter where every lifted test reads the item through a
// method call, which autoderefs to whatever depth it is handed.
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

struct Pending {
    rest: std::vec::IntoIter<&'static str>,
}

// Not flagged: a receiver the closure does not own cannot be moved into
// the leading `filter`, which `E0507` would say.
fn receiver_is_a_field(pending: &mut Pending) -> bool {
    pending
        .rest
        .any(|line| wanted(line) && line.starts_with('#'))
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

// Not flagged: and neither does `&Concrete` to `&dyn Marker`, a user
// trait carrying no blanket impl for a reference.
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

// Not flagged: one mention is not one move. A call in a loop runs again,
// and the second run moves a receiver the first one took.
fn receiver_moved_twice(mut lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut hits = 0;
    for _ in 0..2 {
        if lines.find(|line| wanted(line) && line.starts_with('#')).is_some() {
            hits += 1;
        }
    }
    hits
}

// Not flagged: and the body to count mentions in is the one declaring the
// receiver, not the closure the call sits in.
fn receiver_named_outside(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    let found = (|| lines.find(|line| wanted(line) && line.starts_with('#')))();
    found.is_some() || lines.next().is_some()
}

macro_rules! both {
    ($line:expr) => {
        wanted($line) && $line.starts_with('#')
    };
}

// Not flagged: a conjunction the reader did not write has no `&&` they
// can cut.
fn conjunction_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| both!(line)).collect()
}

// Not flagged: `find` leaves the receiver where it was and the leading
// `filter` would move it, so a receiver named again is `E0382` once
// split.
fn receiver_named_again(
    mut lines: std::vec::IntoIter<&'static str>,
) -> (Option<&'static str>, Option<&'static str>) {
    let first = lines.find(|line| wanted(line) && line.starts_with('#'));
    (first, lines.next())
}

// Bad: a `move` closure's `Copy` capture is one both closures can hold,
// each its own copy, so two tests reaching it still split.
fn copy_capture_in_two_tests(limit: usize, mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(move |line| longer(line, limit) && shorter(line, limit))
}

// Good: each closure holds its own copy of the capture, which is what lets
// both of them read it.
fn split_copy_capture_in_two_tests(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> bool {
    lines
        .filter(move |line| longer(line, limit))
        .any(move |line| shorter(line, limit))
}

fn note(log: &mut Vec<usize>, line: &str) -> bool {
    log.push(line.len());
    !line.is_empty()
}

// Not flagged: two tests reaching a capture held mutably would be two
// closures both holding it.
fn mutable_capture_in_two_tests(lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut log = Vec::new();
    lines
        .filter(|line| note(&mut log, line) && note(&mut log, line.trim()))
        .count()
}

// Not flagged: a destructured parameter, where each test would have to
// reproduce the pattern.
fn destructured(pairs: std::vec::IntoIter<(usize, &'static str)>) -> Vec<(usize, &'static str)> {
    pairs
        .filter(|(key, value)| *key > 0 && wanted(value))
        .collect()
}

// Bad: `Option::filter` keeps whichever values pass rather than stopping at
// one, so its tests lift into a leading `filter` of its own.
fn option_filter(line: Option<&'static str>) -> Option<&'static str> {
    line.filter(|line| wanted(line) && line.starts_with('#'))
}

// Good: one `Option::filter` per test.
fn split_option_filter(line: Option<&'static str>) -> Option<&'static str> {
    line.filter(|line| wanted(line))
        .filter(|line| line.starts_with('#'))
}

// Bad: and so is `Option::is_some_and`.
fn option_is_some_and(line: Option<&'static str>) -> bool {
    line.is_some_and(|line| wanted(line) && line.starts_with('#'))
}

// Good: the leading test filters, leaving the adapter the last one.
fn split_option_is_some_and(line: Option<&'static str>) -> bool {
    line.filter(|line| wanted(line))
        .is_some_and(|line| line.starts_with('#'))
}

// Not flagged: filtering before `is_none_or` makes a value that failed
// the first test vacuously fine, the way it does before `all`.
fn option_is_none_or(line: Option<&'static str>) -> bool {
    line.is_none_or(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: `Result` has no filtering adapter to lift into, so a
// conjunction on one stays folded however it is written.
fn result_is_ok_and(outcome: Result<&'static str, usize>) -> bool {
    outcome.is_ok_and(|line| wanted(line) && line.starts_with('#'))
}

// Bad: a comment inside the predicate does not stop it being two tests, so
// the rule still asks for the split. It offers the rewrite as advice rather
// than applying it, splicing the adapters being what would drop the comment.
fn a_commented_conjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) /* and a heading */ && line.starts_with('#'))
        .collect()
}

// Good: one adapter per test, with the comment kept beside the test it was
// about.
fn split_a_commented_conjunction(
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#') /* a heading */)
        .collect()
}

fn main() {}
