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

fn parse(text: &str) -> Option<usize> {
    text.parse().ok()
}

fn record(length: usize) {}

fn recover(text: &str) -> Result<usize, usize> {
    text.parse().map_err(|_| 0)
}

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

// Not flagged: a method the family's adapters do not include, which has no
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

// Bad: and its continue channel.
fn control_flow_continue(flow: ControlFlow<usize, &'static str>) -> ControlFlow<usize, usize> {
    flow.map_continue(|text| text.trim().len())
}

// Good: one `map_continue` per step.
fn split_control_flow_continue(
    flow: ControlFlow<usize, &'static str>,
) -> ControlFlow<usize, usize> {
    flow.map_continue(str::trim).map_continue(str::len)
}

// Not flagged: a method this family's adapters do not include either.
fn control_flow_broke(flow: ControlFlow<&'static str, usize>) -> bool {
    flow.is_break()
}

// A fallible, defaulted or predicate-shaped adapter takes the item by
// value and hands it back no more than `map` does. The lifted step's
// result stays wrapped in what the adapter keeps, and `runs_of_hex` below
// wants a `mut` rebinding once split, which are the costs `Iterator`'s own
// adapters pay too.

// Bad: a fallible adapter on the value channel.
fn option_and_then(header: Option<&'static str>) -> Option<usize> {
    header.and_then(|text| parse(text.trim()))
}

// Good: the step lifted, leaving the adapter the fallible call.
fn split_option_and_then(header: Option<&'static str>) -> Option<usize> {
    header.map(str::trim).and_then(parse)
}

// Bad: a defaulted adapter, whose closure is its second argument.
fn option_map_or(header: Option<&'static str>) -> usize {
    header.map_or(0, |text| text.trim().len())
}

// Good: the step lifted, leaving the adapter its default.
fn split_option_map_or(header: Option<&'static str>) -> usize {
    header.map(str::trim).map_or(0, str::len)
}

// Bad: and the one whose default is a closure of its own.
fn option_map_or_else(header: Option<&'static str>) -> usize {
    header.map_or_else(|| 0, |text| text.trim().len())
}

// Good: the step lifted, leaving the adapter its default.
fn split_option_map_or_else(header: Option<&'static str>) -> usize {
    header.map(str::trim).map_or_else(|| 0, str::len)
}

// Bad: a predicate-shaped adapter.
fn option_is_some_and(header: Option<&'static str>) -> bool {
    header.is_some_and(|text| text.trim().is_empty())
}

// Good: the step lifted, leaving the adapter its test.
fn split_option_is_some_and(header: Option<&'static str>) -> bool {
    header.map(str::trim).is_some_and(str::is_empty)
}

// Bad: the one that answers the other way round, which a leading `map`
// leaves alone because it turns no `None` into a `Some`.
fn option_is_none_or(header: Option<&'static str>) -> bool {
    header.is_none_or(|text| text.trim().is_empty())
}

// Good: the step lifted, leaving the adapter its test.
fn split_option_is_none_or(header: Option<&'static str>) -> bool {
    header.map(str::trim).is_none_or(str::is_empty)
}

// Bad: a `Result`'s fallible adapter, three steps deep.
fn result_and_then(outcome: Result<&'static str, usize>) -> Result<usize, usize> {
    outcome.and_then(|text| parse(text.trim()).ok_or(0))
}

// Good: every step but the last lifted, leaving the adapter the call that
// turns a `None` into the error.
fn split_result_and_then(outcome: Result<&'static str, usize>) -> Result<usize, usize> {
    outcome
        .map(str::trim)
        .map(parse)
        .and_then(|parsed| parsed.ok_or(0))
}

// Bad: a `Result`'s predicate-shaped adapter.
fn result_is_ok_and(outcome: Result<&'static str, usize>) -> bool {
    outcome.is_ok_and(|text| text.trim().is_empty())
}

// Good: the step lifted, leaving the adapter its test.
fn split_result_is_ok_and(outcome: Result<&'static str, usize>) -> bool {
    outcome.map(str::trim).is_ok_and(str::is_empty)
}

// Bad: a `Result`'s defaulted adapter.
fn result_map_or(outcome: Result<&'static str, usize>) -> usize {
    outcome.map_or(0, |text| text.trim().len())
}

// Good: the step lifted, leaving the adapter its default.
fn split_result_map_or(outcome: Result<&'static str, usize>) -> usize {
    outcome.map(str::trim).map_or(0, str::len)
}

// Bad: one closure per channel, both holding a chain. The table holds one
// adapter per method name, so the value closure is the one read.
fn result_both_channels(outcome: Result<&'static str, &'static str>) -> usize {
    outcome.map_or_else(|text| text.trim().len(), |text| text.trim().len())
}

// Good: a step lifted out of each channel, into the adapter that maps it.
fn split_result_both_channels(outcome: Result<&'static str, &'static str>) -> usize {
    outcome
        .map(str::trim)
        .map_err(str::trim)
        .map_or_else(str::len, str::len)
}

// Bad: the error channel's own adapters, where a lifted step goes into a
// `map_err`.
fn result_or_else(outcome: Result<usize, &'static str>) -> Result<usize, usize> {
    outcome.or_else(|text| recover(text.trim()))
}

// Good: the step lifted into a `map_err`, leaving the adapter the
// recovery.
fn split_result_or_else(outcome: Result<usize, &'static str>) -> Result<usize, usize> {
    outcome.map_err(str::trim).or_else(recover)
}

// Bad: the defaulted one on that channel.
fn result_unwrap_or_else(outcome: Result<usize, &'static str>) -> usize {
    outcome.unwrap_or_else(|text| text.trim().len())
}

// Good: the same one lifted, which is the form the help asks for.
fn split_result_unwrap_or_else(outcome: Result<usize, &'static str>) -> usize {
    outcome.map_err(str::trim).unwrap_or_else(str::len)
}

// Bad: and the predicate-shaped one.
fn result_is_err_and(outcome: Result<usize, &'static str>) -> bool {
    outcome.is_err_and(|text| text.trim().is_empty())
}

// Good: the step lifted into a `map_err`, leaving the adapter its test.
fn split_result_is_err_and(outcome: Result<usize, &'static str>) -> bool {
    outcome.map_err(str::trim).is_err_and(str::is_empty)
}

// Bad: a step whose result is an iterator, which the adapter it lands in
// then has to be handed by a `mut` binding.
fn runs_of_hex(bytes: Option<&'static [u8]>) -> bool {
    bytes.is_some_and(|run| run.iter().all(u8::is_ascii_hexdigit))
}

// Good: the step lifted, with the `mut` that `all` taking `&mut self`
// asks for. That rebinding is the readability cost the split carries here.
fn split_runs_of_hex(bytes: Option<&'static [u8]>) -> bool {
    bytes
        .map(|run| run.iter())
        .is_some_and(|mut run| run.all(u8::is_ascii_hexdigit))
}

// Not flagged: `map_or_else` takes one closure per channel and the table
// holds one adapter per name, so the value one is read and a chain in the
// error one is left alone.
fn result_error_closure(outcome: Result<usize, &'static str>) -> usize {
    outcome.map_or_else(|text| text.trim().len(), |value| value)
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
