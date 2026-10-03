// edition:2024
//
// Which `Option`-returning closures the rule splits, and which it
// leaves alone.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

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

// Bad: a guard welded to a value, which is where the rule's name comes
// from. The value names the item, because a `map` over the item is where
// it goes.
fn guarded(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| wanted(line).then(|| render(line))).collect()
}

// Good: the guard filters and the value maps.
fn split_guarded(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter(|line| wanted(line)).map(|line| render(line)).collect()
}

// Bad: `then_some` is the same guard with the value already in hand.
fn guarded_by_value(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| wanted(line).then_some(1)).collect()
}

// Bad: a consumer, whose answer still depends only on which items
// satisfy the guard.
fn found(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).and_then(validate))
}

// Bad: a prefix-shaped adapter takes the guard, whose lift target is a
// `take_while`.
fn guarded_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(|line| wanted(line).then(|| render(line))).collect()
}

// Good: the guard stops the run and the value maps.
fn split_guarded_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.take_while(|line| wanted(line)).map(|line| render(line)).collect()
}

// Bad: a trailing `map` drops nothing, so it suits a prefix-shaped
// adapter too.
fn infallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(|line| parse(line).map(double)).collect()
}

// Not flagged: a leading `filter_map` would drop the very item that
// would have stopped the run, so this one does not split.
fn fallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).and_then(validate))
        .collect()
}

// Not flagged: a trailing `filter` moves where the run stops, for the
// same reason.
fn filtering_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).filter(positive))
        .collect()
}

// Not flagged: one stage has nothing to hand over.
fn one_stage(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line)).collect()
}

// Not flagged: the second stage names the item, which the adapter it
// would move to never sees.
fn second_stage_names_the_item(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).map(|value| value + line.len()))
        .collect()
}

// Not flagged: a guard naming no item holds for every item or none.
fn invariant_guard(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| flag.then_some(1)).collect()
}

// Not flagged: a `Result` combinator of the same name, whose family has
// no filtering adapter to lift into.
fn result_map(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.trim().parse::<usize>().map(double).ok())
        .collect()
}

fn main() {}
