// edition:2024
//
// Which comparisons this rule reads as an `Option<bool>` measured
// against a state, which rewrite each earns, which forms it asks for
// instead, and which shapes it does not flag.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::collections::HashMap;

struct Entry {
    enabled: Option<bool>,
}

// Bad: one rewrite per operator-and-literal pair.
fn each_pair(verbose: Option<bool>) {
    if verbose == Some(true) {}
    if verbose != Some(false) {}
    if verbose != Some(true) {}
    if verbose == Some(false) {}
}

// Good: `each_pair`'s four comparisons as the rule asks for them. The rule
// does not fire on what it suggests.
fn each_pair_rewritten(verbose: Option<bool>) {
    if verbose.unwrap_or(false) {}
    if verbose.unwrap_or(true) {}
    if !verbose.unwrap_or(false) {}
    if !verbose.unwrap_or(true) {}
}

// Bad: the `Some` on the left reads the same as the `Some` on the right.
fn reversed_operands(entry: Entry) {
    if Some(true) == entry.enabled {}
}

// Bad: a method chain is spliced as it stands.
fn method_chain(flags: HashMap<String, bool>) {
    if flags.get("verbose").copied() == Some(true) {}
}

// Bad: a `&bool` payload, which the rewrite reaches with `copied`.
fn borrowed_payload(flags: HashMap<String, bool>, settings: Option<bool>) {
    if flags.get("verbose") == Some(&true) {}
    if settings.as_ref() != Some(&false) {}
}

// Good: `borrowed_payload`'s two comparisons as the rule asks for them,
// each `copied` reaching the `bool`.
fn borrowed_payload_rewritten(flags: HashMap<String, bool>, settings: Option<bool>) {
    if flags.get("verbose").copied().unwrap_or(false) {}
    if settings.as_ref().copied().unwrap_or(true) {}
}

// Bad: a `&Option<bool>` against a `&Some`; `unwrap_or` resolves through
// the reference, so the rewrite keeps neither.
fn borrowed_option(entry: Entry) {
    let enabled = &entry.enabled;
    if enabled == &Some(true) {}
}

// Bad: an option side whose `&` is written rather than carried by its
// type, which the suggestion takes off rather than bracketing.
fn borrowed_on_both_sides(entry: Entry) {
    if &entry.enabled == &Some(true) {}
}

// Bad: a receiver that binds looser than a method call keeps its
// brackets.
fn dereferenced_option(entry: Entry) {
    let enabled = &entry.enabled;
    if *enabled == Some(true) {}
}

// Bad: a comparison written as a macro argument is the author's own.
fn as_a_macro_argument(verbose: Option<bool>) {
    assert!(verbose == Some(true));
}

// Not flagged: both sides are `Option<bool>`, so neither names a state.
fn two_option_sides(left: Option<bool>, right: Option<bool>) {
    if left == right {}
}

// Not flagged: the `Some` carries a variable, so there is no state to name
// and `unwrap_or` would not be an improvement.
fn a_variable_in_the_some(enabled: Option<bool>, wanted: bool) {
    if enabled == Some(wanted) {}
}

// Not flagged: not an `Option<bool>`.
fn another_payload(version: Option<i32>) {
    if version == Some(3) {}
}

// Not flagged: `matches!` already names the state it matches.
fn a_pattern_match(enabled: Option<bool>) {
    if matches!(enabled, Some(true)) {}
}

// Not flagged: a `&&bool` payload. One `copied` reaches a `&bool` rather
// than the `bool` the comparison answers with, so there is no total
// rewrite to offer.
fn doubly_borrowed_payload(settings: Option<&bool>) {
    if settings.as_ref() == Some(&&true) {}
}

// Not flagged: `assert_eq!` binds both operands before comparing them, so
// the shape never arises -- and the silence is wanted rather than
// tolerated, since the rewrite would cost the failure message the operand
// values it prints.
fn under_an_equality_assertion(verbose: Option<bool>) {
    assert_eq!(verbose, Some(true));
}

// Not flagged: the comparison belongs to the macro, so the suggestion
// would rewrite its definition with one call site's text.
macro_rules! turned_on {
    ($option:expr) => {
        $option == Some(true)
    };
}

fn under_a_local_macro(verbose: Option<bool>) {
    if turned_on!(verbose) {}
}

// Not flagged: the comparison is the author's and only the `Some` is
// expanded, so the rewrite would read the literal's value and drop the
// macro call, freezing today's expansion into the source.
macro_rules! the_wanted_state {
    () => {
        Some(true)
    };
}

fn against_an_expanded_operand(verbose: Option<bool>) {
    if verbose == the_wanted_state!() {}
}

fn main() {}
