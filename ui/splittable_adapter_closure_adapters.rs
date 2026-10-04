// edition:2024
//
// Which adapters the chain trigger speaks about, and where each one's item
// sits in the closure it takes. The condition is that the item enters by
// value and never comes back out, so `filter` and `inspect` are here to be
// left alone, and one case stands for each table entry: an entry naming the
// wrong parameter or the wrong shape would show up as a missing diagnostic.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(text: &str) -> usize {
    text.len()
}

fn record(length: usize) {}

fn maybe(text: &str) -> Option<usize> {
    text.parse().ok()
}

fn label(length: usize) -> &'static str {
    match length {
        0 => "empty",
        _ => "filled",
    }
}



// Bad: a consumer rather than an adapter, whose item still enters by
// value and never comes back out.
fn consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.for_each(|line| record(line.trim().len()));
}



// Good: the steps lifted, leaving the consumer the call that is its own.
fn split_consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.map(str::trim).map(str::len).for_each(record);
}



// Bad: a predicate-returning adapter, where the chain ends in a `bool`.
fn predicate_chain(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| line.trim().is_empty())
}



// Good: the step lifted, leaving the adapter its test. The receiver needs
// no `mut` once the leading `map` takes it by value.
fn split_predicate_chain(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.map(str::trim).any(str::is_empty)
}



// Bad: the last step stays with the adapter, so only the steps a
// leading `map` would take have to lift. `label`'s result borrows, and
// an owned item could not hand that back out of a `map`.
fn last_step_borrows(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| label(header.len())).collect()
}



// Good: the step before the last lifted, leaving the adapter the call
// whose result borrows.
fn split_last_step_borrows(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| header.len()).map(label).collect()
}



// Bad: `filter_map`, `find_map` and `map_while` meet the same condition,
// and a body that is a chain rather than `Option` work lifts into a
// leading `map` as any other adapter's does. The guard-and-value trigger
// answers first where the body is `Option` work.
fn parsing(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.trim().parse::<usize>().ok())
        .collect()
}



fn first_parsed(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| line.trim().parse::<usize>().ok())
}



fn parsed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| line.trim().parse::<usize>().ok())
        .collect()
}



// Good: one adapter per step, each of the three adapters left what it is.
fn split_parsing(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .filter_map(Result::ok)
        .collect()
}



fn split_first_parsed(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map(Result::ok)
}



fn split_parsed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .map_while(Result::ok)
        .collect()
}



// Not flagged: `filter` hands the item back downstream, so a leading
// `map` would change what every later adapter sees.
fn filtering(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| line.trim().is_empty()).collect()
}



// Not flagged: `inspect` passes the item on unchanged, for the same
// reason.
fn inspecting(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.inspect(|line| record(line.trim().len())).collect()
}



trait Mapper {
    fn map<Output>(self, body: impl FnOnce(&'static str) -> Output) -> Output;
}



impl Mapper for &'static str {
    fn map<Output>(self, body: impl FnOnce(&'static str) -> Output) -> Output {
        body(self)
    }
}



// Not flagged: nor is a trait of one's own, whose method of that name
// says nothing about how the item arrives.
fn another_trait_map(header: &'static str) -> usize {
    header.map(|text| text.trim().len())
}



// Bad: a binary closure, whose accumulator stays in the body while the
// whole chain lifts.
fn binary_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len())
}



// Good: the same fold with its steps lifted out.
fn split_binary_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .fold(0, |total, length| total + length)
}



// Bad: the rest of the adapter table, one case each, so an entry
// naming the wrong parameter or the wrong shape would show up here.
fn flattening(lines: std::vec::IntoIter<&'static str>) -> String {
    lines.flat_map(|line| line.trim().chars()).collect()
}



fn trying_each(mut lines: std::vec::IntoIter<&'static str>) -> Option<()> {
    lines.try_for_each(|line| Some(record(line.trim().len())))
}



fn every(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.all(|line| line.trim().is_empty())
}



fn first_empty(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.position(|line| line.trim().is_empty())
}



fn last_empty(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.rposition(|line| line.trim().is_empty())
}



fn try_total(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.try_fold(0, |total, line| Some(total + line.trim().len()))
}



fn total_from_the_right(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.rfold(0, |total, line| total + line.trim().len())
}



fn try_total_from_the_right(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.try_rfold(0, |total, line| Some(total + line.trim().len()))
}



fn running_total(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .scan(0, |total, line| Some(*total + line.trim().len()))
        .collect()
}


// Good: the same table, split. Each adapter is left what it is, and the
// stateful ones keep the accumulation alone.
fn split_flattening(lines: std::vec::IntoIter<&'static str>) -> String {
    lines.map(str::trim).flat_map(str::chars).collect()
}


fn split_trying_each(lines: std::vec::IntoIter<&'static str>) -> Option<()> {
    lines
        .map(str::trim)
        .map(str::len)
        .map(record)
        .try_for_each(Some)
}


fn split_every(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.map(str::trim).all(str::is_empty)
}


fn split_first_empty(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.map(str::trim).position(str::is_empty)
}


fn split_last_empty(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.map(str::trim).rposition(str::is_empty)
}


fn split_try_total(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines
        .map(str::trim)
        .map(str::len)
        .try_fold(0, |total, length| Some(total + length))
}


fn split_total_from_the_right(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .rfold(0, |total, length| total + length)
}


fn split_try_total_from_the_right(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines
        .map(str::trim)
        .map(str::len)
        .try_rfold(0, |total, length| Some(total + length))
}


fn split_running_total(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::len)
        .scan(0, |total, length| Some(*total + length))
        .collect()
}

fn main() {}
