// aux-build:callback_macros.rs
// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// What keeps this rule off proc-macro output is the keyed lookup in
// `src/rules/multi_statement_iterator_closure/statements.rs`: a range
// is recorded only where the source shows a closure passed as the final
// argument of a call to one of the methods the rule covers, so a
// callback is measured only where the author wrote one. Borrowing a
// span is not enough, because the span borrowed has to cover a callback
// the author already wrote. The cases below pin that from both sides.
//
// `#[derive(SynthCallback)]` emits a two-statement callback whose every
// token wears the user span of `#[synth_callback]`, so a guard that
// asks whether the enclosing item came from a macro has nothing to
// find. Nothing is reported for it.
//
// `#[rebuilt_signature]` is the opposite shape, and the one a guard
// like that gets wrong. The macro re-emits the `fn` keyword as its own
// token and passes the body through. The item's span therefore carries
// the expansion. The callback inside it is the caller's own source,
// down to the byte. That callback is reported. `#[verbatim]` is the
// control, and the swap between the two is what shows the signature
// token is the only difference.
//
// `#[respan_onto_paren]` is one way past a byte-range lookup. It
// stamps synthesised code onto the span of a parenthesis group the
// caller wrote. The synthesised callback therefore lands on the key a
// parenthesised callback is written under. What rejects it is that
// every token it emits carries that one span, leaving the call
// starting no earlier than its own argument, which no call the author
// wrote ever does. Nothing is reported for it, and the diagnostic it
// would otherwise carry names a `let`-bound closure in a function
// holding no iterator call at all.
//
// `#[wrap_in_iterator_call]` needs no span trickery whatever, which is
// what makes it the one to care about. It wraps a closure the caller
// wrote in a synthesised iterator call. The synthesised tokens keep the
// `Span::call_site()` that `quote!` hands them. The
// callback's byte range is the caller's, so it is keyed and its own
// span answers nothing. The call is what came from the expansion, and
// asking that of the call's span is what rejects it. Nothing is
// reported for it either.
//
// `#[respan_two_targets]` is the same trick with two targets. The
// synthesised call takes one span the caller wrote and the synthesised
// callback another. No comparison between the two spans separates it
// from real code. Reading the source does, since neither span covers a
// callback the author passed to anything. Nothing is reported for it.
//
// `#[synthesise_receiver]` and `#[rebuild_argument_group]` come at it
// from the other side, and are why asking where the *call* came from
// does not work. A call's span runs from its receiver to its closing
// parenthesis, and combining spans of differing syntax context yields
// the macro's. So substituting the receiver is enough to make an
// author-written call look macro-made, and so is merely rebuilding the
// argument list's parentheses, leaving every token the caller's. Both
// callbacks are the caller's own, and both are reported.
//
// `#[substitute_callee]` is why the name recorded is the one in the
// source. The caller wrote `helper(..)` with a closure as its final
// argument, which a record keyed on shape alone would accept once the
// macro makes the call resolve to `Iterator::map`; keyed on the written
// name it is not accepted, because `helper` is not a method the rule
// covers. Nothing is reported for it.

extern crate callback_macros;

use callback_macros::{
    SynthCallback, rebuild_argument_group, rebuilt_signature, respan_onto_paren,
    respan_two_targets, substitute_callee, synthesise_receiver, verbatim,
    wrap_in_iterator_call,
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

#[respan_two_targets]
fn the_caller_wrote_a_parenthesised_closure_twice() {
    let identity = (|value: i32| {
        let doubled = value * 2;
        doubled
    });
}

#[synthesise_receiver]
fn the_receiver_came_from_the_macro() {
    let _ = ITEMS.map(|value| {
        let doubled = value * 2;
        doubled
    });
}

#[rebuild_argument_group]
fn the_argument_parentheses_lost_their_span() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
}

#[substitute_callee]
fn the_caller_wrote_a_call_to_something_else() {
    let _ = helper(|value| {
        let doubled = value * 2;
        doubled
    });
}

fn main() {}
