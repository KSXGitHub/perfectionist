// edition:2024
//
// Which combinator the `Option` work in the closure ends in, and what
// the split hands that combinator's work to. The adapter is `filter_map`
// throughout, which drops what the closure answers `None` with and
// yields a stream, so every lift target here is the plain one.
//
// Each Bad case is followed by the Good one the rule's help asks for,
// which is where a form the rule asks for and then fires on again would
// show up.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(line: &str) -> Option<usize> {
    line.trim().parse().ok()
}

fn validate(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}

fn double(value: usize) -> usize {
    value * 2
}

fn positive(value: &usize) -> bool {
    *value > 0
}

fn wanted(line: &str) -> bool {
    !line.is_empty()
}

fn render(line: &str) -> usize {
    line.len()
}

// Bad: two fallible stages welded together.
fn fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line).and_then(validate)).collect()
}

// Good: one adapter per stage, which is what the rule asks for.
fn split_fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).filter_map(validate).collect()
}

// Bad: a fallible stage and an infallible one.
fn infallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line).map(double)).collect()
}

// Good: the same with the infallible half trailing.
fn split_infallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).map(double).collect()
}

// Bad: a fallible stage and a test.
fn filtering(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line).filter(positive)).collect()
}

// Good: the same with the test trailing.
fn split_filtering(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).filter(positive).collect()
}

// Bad: a guard welded to a value, which is what this trigger is named
// for. The value names the item, because a `map` over the item is where it
// goes.
fn guarded(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| wanted(line).then(|| render(line))).collect()
}

// Good: the guard filters and the value maps.
fn split_guarded(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter(|line| wanted(line)).map(|line| render(line)).collect()
}

// Bad: `then_some` holds the guard with the value already in hand rather
// than in a closure.
fn guarded_by_value(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| wanted(line).then_some(1)).collect()
}

// Good: the guard filters and the value maps, as it does for `then`.
fn split_guarded_by_value(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter(|line| wanted(line)).map(|_| 1).collect()
}

fn main() {}
