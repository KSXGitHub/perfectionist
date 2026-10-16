// edition:2024
//
// A closure holding no `Option` work the trigger can hand over: a
// combinator the reader did not write, a single stage, a stage naming the
// item the next adapter never sees, a guard naming no item, an adapter
// with no `Option` work to give away, and an outermost call taking no
// argument.
//
// The steps may still split where the stages cannot, so a Bad case here
// is followed by the Good form the chain trigger asks for.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(line: &str) -> Option<usize> {
    line.trim().parse().ok()
}

fn double(value: usize) -> usize {
    value * 2
}

macro_rules! staged {
    ($line:expr) => {
        parse($line).map(double)
    };
}

// Not flagged: a combinator the reader did not write has no stage
// boundary they can move.
fn stage_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| staged!(line)).collect()
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

// Bad: `Iterator::map` has no `Option` work to hand over, so only the
// steps split, which is the chain trigger's finding.
fn plain_map(lines: std::vec::IntoIter<&'static str>) -> Vec<Option<usize>> {
    lines.map(|line| parse(line).map(double)).collect()
}

// Good: the step lifted, leaving the closure the `Option` work.
fn split_plain_map(lines: std::vec::IntoIter<&'static str>) -> Vec<Option<usize>> {
    lines.map(parse).map(|parsed| parsed.map(double)).collect()
}

// Bad: the closure's outermost call takes no argument, so the
// guard-and-value trigger has no second stage to hand over. The chain
// trigger reads the same body as four steps.
fn no_second_stage(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.trim().parse::<usize>().map(double).ok())
        .collect()
}

// Good: every step but the last lifted, leaving the adapter the call that
// discards the error.
fn split_no_second_stage(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .map(|parsed| parsed.map(double))
        .filter_map(Result::ok)
        .collect()
}

fn main() {}
