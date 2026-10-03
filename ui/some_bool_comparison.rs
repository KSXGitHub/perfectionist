// edition:2024
//
// Which comparisons this rule reads as an `Option<bool>` measured
// against a state, which rewrite each earns, and which shapes it leaves
// alone.

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

// Bad: a `&Option<bool>` against a `&Some`; `unwrap_or` resolves through
// the reference, so the rewrite keeps neither.
fn borrowed_option(entry: Entry) {
    let enabled = &entry.enabled;
    if enabled == &Some(true) {}
}

// Bad: the same with the option side's reference written out, which the
// suggestion takes off rather than bracketing.
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

fn not_flagged(left: Option<bool>, right: Option<bool>, wanted: bool, version: Option<i32>) {
    // Both sides are `Option<bool>`, so neither names a state.
    if left == right {}
    // The `Some` carries a variable: no state to name, and `unwrap_or`
    // would not be an improvement.
    if left == Some(wanted) {}
    // Not an `Option<bool>`.
    if version == Some(3) {}
    // Already names the state it matches.
    if matches!(left, Some(true)) {}
}

// Not flagged: a `&&bool` payload. One `copied` reaches a `&bool` rather
// than the `bool` the comparison answers with, so there is no total
// rewrite to offer.
fn doubly_borrowed_payload(settings: Option<&bool>) {
    if settings.as_ref() == Some(&&true) {}
}

// Not flagged, and not an oversight: `assert_eq!` binds both operands
// before comparing them, so the shape never arises -- and the silence is
// wanted, since the rewrite would cost the failure message the operand
// values it prints.
fn under_an_equality_assertion(verbose: Option<bool>) {
    assert_eq!(verbose, Some(true));
}

// Not flagged, and not an oversight either: the comparison belongs to the
// macro, so the suggestion would rewrite its definition with one call
// site's text.
macro_rules! turned_on {
    ($option:expr) => {
        $option == Some(true)
    };
}

fn under_a_local_macro(verbose: Option<bool>) {
    if turned_on!(verbose) {}
}

// Not flagged, for its own reason: the comparison is the author's and
// only the `Some` is expanded, so the rewrite would read the literal's
// value and drop the macro call, freezing today's expansion into the
// source.
macro_rules! the_wanted_state {
    () => {
        Some(true)
    };
}

fn against_an_expanded_operand(verbose: Option<bool>) {
    if verbose == the_wanted_state!() {}
}

fn main() {}
