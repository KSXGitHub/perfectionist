// aux-build:callback_macros.rs
// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// What keeps this rule off proc-macro output is the span-keyed lookup
// in `src/rules/multi_statement_iterator_closure/statements.rs`: a
// callback is measured only where its byte range matches a closure in
// a re-parsed module file, which synthesised code does not occupy. The
// two halves of this fixture pin that from both sides.
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

extern crate callback_macros;

use callback_macros::{SynthCallback, rebuilt_signature, verbatim};

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

fn main() {}
