// aux-build:callback_macros.rs
// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// What keeps this rule off proc-macro output is mostly the span-keyed
// lookup in `src/rules/multi_statement_iterator_closure/statements.rs`:
// a callback is measured only where its byte range matches a closure in
// a re-parsed module file, which synthesised code does not occupy. Where
// a macro can put synthesised code on a range the caller wrote, the two
// questions in `check_expr` decide it instead. The cases below pin that
// from both sides.
//
// `#[derive(SynthCallback)]` emits a two-statement callback whose every
// token wears the user span of `#[synth_callback]`, so a guard that
// asks whether the enclosing item came from a macro has nothing to
// find. Nothing is reported for it.
//
// `#[rebuilt_signature]` is the opposite shape, and the one a guard
// like that gets wrong: the macro re-emits the `fn` keyword as its own
// token and passes the body through, so the item's span carries the
// expansion while the callback inside it is the caller's own source,
// down to the byte. That callback is reported. `#[verbatim]` is the
// control, and the swap between the two is what shows the signature
// token is the only difference.
//
// `#[respan_onto_paren]` is one way past a byte-range lookup: it
// stamps synthesised code onto the span of a parenthesis group the
// caller wrote, so the synthesised callback lands on the key a
// parenthesised callback is written under. What rejects it is that
// every token it emits carries that one span, leaving the call
// starting no earlier than its own argument, which no call the author
// wrote ever does. Nothing is reported for it, and the diagnostic it
// would otherwise carry names a `let`-bound closure in a function
// holding no iterator call at all.
//
// `#[wrap_in_iterator_call]` needs no span trickery whatever, which is
// what makes it the one to care about: it wraps a closure the caller
// wrote in a synthesised iterator call and leaves the synthesised
// tokens on the `Span::call_site()` that `quote!` hands them. The
// callback's byte range is the caller's, so it is keyed and its own
// span answers nothing -- the call is what came from the expansion,
// and asking that of the call's span is what rejects it. Nothing is
// reported for it either.
//
// One shape is left over. A macro that re-spans a synthesised call
// onto a token the caller wrote, earlier than the callback, answers
// both questions the way author-written code does. It takes two
// deliberate re-span targets, no `quote!` produces it, and pinning it
// here would mean committing the wrong answer as the expected one, so
// it is named rather than fixtured.

extern crate callback_macros;

use callback_macros::{
    SynthCallback, rebuilt_signature, respan_onto_paren, verbatim, wrap_in_iterator_call,
};

#[derive(SynthCallback)]
#[synth_callback]
struct SynthesisedCallback;

#[rebuilt_signature]
fn signature_came_from_the_macro() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
}

#[verbatim]
fn nothing_came_from_the_macro() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
}

#[respan_onto_paren]
fn the_caller_wrote_a_parenthesised_closure() {
    let identity = (|value: i32| {
        let doubled = value * 2;
        doubled
    });
}

#[wrap_in_iterator_call]
fn the_caller_wrote_a_plain_closure() {
    let identity = |value| {
        let doubled = value * 2;
        doubled
    };
}

fn main() {}
