// edition:2024
//
// Closures whose `Option` work runs more than the two stages each shape
// of the guard-and-value split is named for, and what the rule tells the
// reader to do about them.
//
// A chain of seams is reported by its stage count, because the count is
// what the rewrite comes to: one adapter per stage. The help has to name
// every seam, since a reader who follows it and is reported again was
// given advice that did not finish.
//
// Every Bad case is followed by the Good one the rule's help asks for,
// which is where that finishing is checked.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(line: &str) -> Option<usize> {
    line.trim().parse().ok()
}

fn validate(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}

fn narrow(value: usize) -> Option<usize> {
    (value < 100).then_some(value)
}

fn double(value: usize) -> usize {
    value * 2
}

fn bump(value: usize) -> usize {
    value + 1
}

fn positive(value: &usize) -> bool {
    *value > 0
}

fn small(value: &usize) -> bool {
    *value < 100
}

// Bad: three fallible stages welded together, one more than the pair the
// shape is named for.
fn three_fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).and_then(validate).and_then(narrow))
        .collect()
}

// Good: one adapter per stage, which comes to three.
fn split_three_fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(parse)
        .filter_map(validate)
        .filter_map(narrow)
        .collect()
}

// Bad: four stages, so the count the diagnostic names is read off the
// chain rather than standing in for "more than two".
fn four_fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).and_then(validate).and_then(narrow).and_then(validate))
        .collect()
}

// Good: one adapter per stage, which comes to four.
fn split_four_fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(parse)
        .filter_map(validate)
        .filter_map(narrow)
        .filter_map(validate)
        .collect()
}

// Bad: a fallible stage and two values made after it.
fn two_values(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).map(double).map(bump))
        .collect()
}

// Good: the stage that may fail keeps the `filter_map`, and each value
// gets a `map`.
fn split_two_values(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).map(double).map(bump).collect()
}

// Bad: a value tested twice in the closure that made it.
fn two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).filter(positive).filter(small))
        .collect()
}

// Good: the stage that makes the value keeps the `filter_map`, and each
// test trails it.
fn split_two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(parse)
        .filter(positive)
        .filter(small)
        .collect()
}

// Bad: a value made and then tested, whose two seams lift into different
// adapters, so naming one adapter would leave the other seam unaccounted
// for.
fn unalike_seams(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).map(double).filter(positive))
        .collect()
}

// Good: the stage that may fail keeps the `filter_map`, the value maps
// and the test filters.
fn split_unalike_seams(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(parse)
        .map(double)
        .filter(positive)
        .collect()
}

// Bad: three fallible stages under a one-value adapter, whose only
// trailing adapter is the `Option`'s, so each lift is a leading one.
fn three_found(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).and_then(validate).and_then(narrow))
}

// Good: a leading `filter_map` for each stage before the search, leaving
// `find_map` the last of them.
fn split_three_found(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.filter_map(parse).filter_map(validate).find_map(narrow)
}

// Bad: three stages under a prefix-shaped adapter, which takes them
// because a trailing `map` drops nothing and so leaves the run stopping
// where it did.
fn three_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).map(double).map(bump))
        .collect()
}

// Good: the stage that may fail keeps the `map_while`, and each value
// trails it.
fn split_three_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(parse).map(double).map(bump).collect()
}

// Bad: the inner stage borrows what the item points at, so lifting that
// stage is `E0515` and the count stops at the seam above it. Reporting
// the two-stage shape is what holds the help to a split that compiles.
fn inner_stage_borrows(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.strip_prefix("# ").map(str::len).map(double))
        .collect()
}

// Good: the borrowing stage carried by a leading `map` as a step rather
// than lifted as a seam, which leaves each later stage an adapter of its
// own.
fn split_inner_stage_borrows(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.strip_prefix("# "))
        .filter_map(|held| held.map(str::len))
        .map(double)
        .collect()
}

// Not flagged: one adapter per stage already, which is where the split of
// a three-stage closure arrives.
fn already_split(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(parse)
        .filter_map(validate)
        .filter_map(narrow)
        .collect()
}

fn main() {}
