// edition:2024
//
// Which adapter the split hands the work to, which the outer adapter
// decides rather than the closure. `find_map` yields one value, so the
// only trailing adapter it has is the `Option`'s own, which rules out
// lifting work to its right. `map_while` stops the run, so its lift
// targets are the ones that stop there too: a `map_while` for a stage
// that may fail and a `take_while` for a test, where a set-shaped
// adapter takes a `filter_map` and a `filter`.
//
// Each Bad case is followed by the Good one the rule's help asks for, so
// a lift target named for the wrong adapter shows up as a case that
// fires again.

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

// Bad: a consumer rather than an adapter, whose answer depends only on
// which items satisfy the guard.
fn found(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).and_then(validate))
}

// Good: the stage before the `and_then` takes a leading `filter_map`,
// because the only trailing adapter a one-value answer has is the
// `Option`'s own.
fn split_found(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.filter_map(parse).find_map(validate)
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

// Bad: a trailing `map` drops nothing, which is what suits it to a
// prefix-shaped adapter, whose run no dropped item may shorten.
fn infallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(|line| parse(line).map(double)).collect()
}

// Good: the `map` becomes a trailing one, which drops nothing and so
// leaves the run stopping where it did.
fn split_infallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(parse).map(double).collect()
}

// Bad: two fallible stages welded together under a prefix-shaped
// adapter, whose lift target for a stage is a `map_while` of its own: a
// `filter_map` there would drop the very item the run would have stopped
// at.
fn fallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).and_then(validate))
        .collect()
}

// Good: one `map_while` per fallible stage, each stopping the run where
// the folded form stopped it.
fn split_fallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(parse).map_while(validate).collect()
}

// Bad: a stage and a test of what it made, under a prefix-shaped adapter
// whose lift target for a test is a `take_while`: a trailing `filter`
// there would drop the item the run would have stopped at.
fn filtering_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).filter(positive))
        .collect()
}

// Good: the stage keeps the `map_while` and the test trails it in a
// `take_while`, which stops the run where the folded form stopped it.
fn split_filtering_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(parse).take_while(positive).collect()
}

// Bad: `find_map` yields one value, so the only trailing `filter` it has
// is the `Option`'s, which rejects what the search settled on where the
// folded form kept looking. The chain trigger lifts the stage before it
// instead, which keeps the search.
fn found_filtering(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).filter(positive))
}

// Good: the stage before the `filter` lifted, which keeps the search
// looking where a trailing `Option::filter` would have settled.
fn split_found_filtering(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.map(parse).find_map(|parsed| parsed.filter(positive))
}

// Not flagged: a one-value adapter has nowhere to put a guard's value, that
// value being gone once a leading adapter has made a stream of them.
fn found_guarded(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| wanted(line).then(|| render(line)))
}

// Bad: a trailing `map` drops nothing, so a one-value adapter takes it.
fn found_infallible(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).map(double))
}

// Good: the `map` becomes a trailing one, on the `Option` the search
// answered with.
fn split_found_infallible(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(parse).map(double)
}

fn main() {}
