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
fn split_pair(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
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

// Bad: a disjunction inside a conjunct stays whole, and the pair still
// splits.
fn conjunct_holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && (line.starts_with('#') || line.ends_with('!')))
        .collect()
}

// Bad: a consumer whose answer still depends only on which items
// satisfy the predicate.
fn find_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.find(|line| wanted(line) && line.starts_with('#'))
}

// Bad: the same from the other end.
fn rfind_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.rfind(|line| wanted(line) && line.starts_with('#'))
}

// Bad: `any` takes its item by value where the lifted `filter` hands a
// reference, and a `&&str` derefs to where these tests read it.
fn any_tests(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| wanted(line) && line.starts_with('#'))
}

// Bad: a prefix-shaped adapter, whose tests lift into a `take_while`
// rather than a `filter`.
fn take_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .take_while(|line| wanted(line) && line.starts_with('#'))
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

// Not flagged: `Option::is_some_and` hands it over the same way.
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

// Not flagged: `find` leaves the receiver where it was and the leading
// `filter` would move it, so a receiver named again is `E0382` once
// split.
fn receiver_named_again(
    mut lines: std::vec::IntoIter<&'static str>,
) -> (Option<&'static str>, Option<&'static str>) {
    let first = lines.find(|line| wanted(line) && line.starts_with('#'));
    (first, lines.next())
}

// Not flagged: a conjunction of comparisons is one test. Each of these
// asks a single question that its halves do not: whether a byte is
// whitespace, and whether a position is inside a range.
fn whitespace_byte(bytes: std::slice::Iter<'static, u8>) -> bool {
    bytes.clone().any(|byte| *byte != b' ' && *byte != b'\t')
}

fn inside_a_range(positions: std::vec::IntoIter<usize>, start: usize, end: usize) -> bool {
    positions.into_iter().any(|pos| pos >= start && pos < end)
}

// Not flagged: one comparison among the conjuncts is enough, since the
// rule cannot tell which bound belongs to which question.
fn a_comparison_among_calls(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| line.len() > 3 && wanted(line)).collect()
}

// Not flagged: two tests reaching a capture held mutably would be two
// closures both holding it.
fn note(log: &mut Vec<usize>, line: &str) -> bool {
    log.push(line.len());
    !line.is_empty()
}

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

// Bad: `Option::filter` is set-shaped too, so its tests lift into a
// leading `filter` of its own.
fn option_filter(line: Option<&'static str>) -> Option<&'static str> {
    line.filter(|line| wanted(line) && line.starts_with('#'))
}

// Bad: and so is `Option::is_some_and`.
fn option_is_some_and(line: Option<&'static str>) -> bool {
    line.is_some_and(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: filtering before `is_none_or` makes a value that failed
// the first test vacuously fine, the way it does before `all`.
fn option_is_none_or(line: Option<&'static str>) -> bool {
    line.is_none_or(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: `Result` has no filtering adapter to lift into, so these
// stay folded however they are written.
fn result_is_ok_and(outcome: Result<&'static str, usize>) -> bool {
    outcome.is_ok_and(|line| wanted(line) && line.starts_with('#'))
}

fn main() {}
