// edition:2024
//
// The families beyond `Iterator`, each with its own lift target.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ops::ControlFlow;
use std::task::Poll;

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

// Good: one adapter per step.
fn split_result_value(outcome: Result<&'static str, usize>) -> Result<usize, usize> {
    outcome.map(str::trim).map(str::len)
}

// Bad: a `Result`'s error channel, where a lifted step goes into a
// `map_err` instead.
fn result_error(outcome: Result<usize, &'static str>) -> Result<usize, usize> {
    outcome.map_err(|text| text.trim().len())
}

// Good: one `map_err` per step, which is where `map_err` comes in.
fn split_result_error(outcome: Result<usize, &'static str>) -> Result<usize, usize> {
    outcome.map_err(str::trim).map_err(str::len)
}

// Bad: a `Poll`'s value channel.
fn poll_value(state: Poll<&'static str>) -> Poll<usize> {
    state.map(|text| text.trim().len())
}

// Good: one adapter per step.
fn split_poll_value(state: Poll<&'static str>) -> Poll<usize> {
    state.map(str::trim).map(str::len)
}

// Bad: a `Poll`'s `Ok` channel.
fn poll_ok(state: Poll<Result<&'static str, usize>>) -> Poll<Result<usize, usize>> {
    state.map_ok(|text| text.trim().len())
}

// Good: one `map_ok` per step, the channel the item sits in.
fn split_poll_ok(state: Poll<Result<&'static str, usize>>) -> Poll<Result<usize, usize>> {
    state.map_ok(str::trim).map_ok(str::len)
}

// Bad: a `Poll`'s `Err` channel, where a lifted step goes into a
// `map_err` of its own rather than the `map_ok` the value channel takes.
fn poll_err(state: Poll<Result<usize, &'static str>>) -> Poll<Result<usize, usize>> {
    state.map_err(|text| text.trim().len())
}

// Good: one `map_err` per step.
fn split_poll_err(state: Poll<Result<usize, &'static str>>) -> Poll<Result<usize, usize>> {
    state.map_err(str::trim).map_err(str::len)
}

// Not flagged: a method `Poll`'s adapters do not include, which has no
// channel for a lifted step to go into.
fn poll_readiness(state: Poll<&'static str>) -> bool {
    state.is_ready()
}

// Bad: a `ControlFlow` break channel.
fn control_flow_break(flow: ControlFlow<&'static str, usize>) -> ControlFlow<usize, usize> {
    flow.map_break(|text| text.trim().len())
}

// Good: one `map_break` per step.
fn split_control_flow_break(flow: ControlFlow<&'static str, usize>) -> ControlFlow<usize, usize> {
    flow.map_break(str::trim).map_break(str::len)
}

// Bad: the continue channel, whose `map_continue` maps the value it
// carries.
fn control_flow_continue(flow: ControlFlow<usize, &'static str>) -> ControlFlow<usize, usize> {
    flow.map_continue(|text| text.trim().len())
}

// Good: one `map_continue` per step.
fn split_control_flow_continue(
    flow: ControlFlow<usize, &'static str>,
) -> ControlFlow<usize, usize> {
    flow.map_continue(str::trim).map_continue(str::len)
}

// Not flagged: a method `ControlFlow`'s adapters do not include, which has
// no channel for a lifted step to go into.
fn control_flow_broke(flow: ControlFlow<&'static str, usize>) -> bool {
    flow.is_break()
}

fn main() {}
