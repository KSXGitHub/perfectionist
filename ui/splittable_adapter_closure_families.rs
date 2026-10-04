// edition:2024
//
// The families beyond `Iterator`, each with its own lift target.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ops::ControlFlow;
use std::task::Poll;

fn parse(text: &str) -> Option<usize> {
    text.parse().ok()
}

fn record(length: usize) {}

// Bad: an `Option`'s value channel, where a lifted step goes into a
// `map`.
fn option_value(header: Option<&'static str>) -> Option<usize> {
    header.map(|text| text.trim().len())
}

// Good: one adapter per step.
fn split_option_value(header: Option<&'static str>) -> Option<usize> {
    header.map(str::trim).map(str::len)
}

// Bad: a `Result`'s value channel.
fn result_value(outcome: Result<&'static str, usize>) -> Result<usize, usize> {
    outcome.map(|text| text.trim().len())
}

// Bad: a `Result`'s error channel, where a lifted step goes into a
// `map_err` instead.
fn result_error(outcome: Result<usize, &'static str>) -> Result<usize, usize> {
    outcome.map_err(|text| text.trim().len())
}

// Good: the error channel split, which is where `map_err` comes in.
fn split_result_error(outcome: Result<usize, &'static str>) -> Result<usize, usize> {
    outcome.map_err(str::trim).map_err(str::len)
}

// Bad: a `Poll`'s value channel.
fn poll_value(state: Poll<&'static str>) -> Poll<usize> {
    state.map(|text| text.trim().len())
}

// Bad: a `Poll`'s `Ok` channel.
fn poll_ok(state: Poll<Result<&'static str, usize>>) -> Poll<Result<usize, usize>> {
    state.map_ok(|text| text.trim().len())
}

// Bad: a `ControlFlow` break channel.
fn control_flow_break(flow: ControlFlow<&'static str, usize>) -> ControlFlow<usize, usize> {
    flow.map_break(|text| text.trim().len())
}

// Bad: and its continue channel.
fn control_flow_continue(flow: ControlFlow<usize, &'static str>) -> ControlFlow<usize, usize> {
    flow.map_continue(|text| text.trim().len())
}

// Not flagged: a fallible, defaulted or predicate-shaped adapter keeps
// the lifted step's result wrapped in what it returns, so the split
// costs a wrapper, and `all` below costs a `mut` rebinding as well. Each
// of these is equivalent split; none of them reads better.
fn option_and_then(header: Option<&'static str>) -> Option<usize> {
    header.and_then(|text| parse(text.trim()))
}

fn option_map_or(header: Option<&'static str>) -> usize {
    header.map_or(0, |text| text.trim().len())
}

fn option_is_some_and(header: Option<&'static str>) -> bool {
    header.is_some_and(|text| text.trim().is_empty())
}

fn result_both_channels(outcome: Result<&'static str, &'static str>) -> usize {
    outcome.map_or_else(|text| text.trim().len(), |text| text.trim().len())
}

fn runs_of_hex(bytes: Option<&'static [u8]>) -> bool {
    bytes.is_some_and(|run| run.iter().all(u8::is_ascii_hexdigit))
}

// Not flagged: `Option::filter` hands the value back, so a leading `map`
// would change what it sees.
fn option_filter(header: Option<&'static str>) -> Option<&'static str> {
    header.filter(|text| text.trim().is_empty())
}

// Not flagged: `Option::inspect` passes the value on unchanged.
fn option_inspect(header: Option<&'static str>) -> Option<&'static str> {
    header.inspect(|text| record(text.trim().len()))
}

// Not flagged: a closure handed nothing has no item to root a chain at.
fn option_or_else(header: Option<usize>, fallback: &'static str) -> Option<usize> {
    header.or_else(|| Some(fallback.trim().len()))
}

// Not flagged: one step is already one adapter doing one thing.
fn one_step(header: Option<&'static str>) -> Option<&'static str> {
    header.map(|text| text.trim())
}

fn main() {}
