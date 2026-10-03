// edition:2024
//
// Which comparisons this rule reads as an `Option<bool>` measured
// against a state, which rewrite each earns, and which shapes name
// their state already.

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

// Bad: the `&` on the `Some` and the one on the option side both come
// off, since `unwrap_or` resolves through either.
fn borrowed_option(entry: Entry) {
    let enabled = &entry.enabled;
    if enabled == &Some(true) {}
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

fn already_named(left: Option<bool>, right: Option<bool>, wanted: bool, version: Option<i32>) {
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

// Deliberately silent, not an oversight: `assert_eq!` builds the
// comparison itself, so the rewrite would cost the failure message the
// operand values it prints, and the assertion already names the state it
// expects.
fn under_an_equality_assertion(verbose: Option<bool>) {
    assert_eq!(verbose, Some(true));
}

// Deliberately silent for the same reason, one crate nearer: the
// comparison belongs to the macro, so the suggestion would rewrite its
// definition with one call site's text.
macro_rules! turned_on {
    ($option:expr) => {
        $option == Some(true)
    };
}

fn under_a_local_macro(verbose: Option<bool>) {
    if turned_on!(verbose) {}
}

fn main() {}
