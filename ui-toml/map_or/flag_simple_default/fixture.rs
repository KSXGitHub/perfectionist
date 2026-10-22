// edition:2024
#![allow(dead_code, unused, reason = "ui fixture")]

// `exempt_simple_default = false` takes the exemption out, so a
// literal or a path default is flagged like any other. Every default
// that collapses the call into a shorter one of the receiver's own is
// simple, so this is also where those helps are reachable.

// Bad: the default is a literal, which the exemption would have taken.
fn literal_default(width: Option<u32>) -> u32 {
    width.map_or(80, |width| width.saturating_sub(2))
}

// Bad: `0` is what `u32`'s `Default` gives, so the fix rewrites the
// call as `map_or_default`.
fn zero_default(width: Option<u32>) -> u32 {
    width.map_or(0, |width| width + 1)
}

// Bad: defaulting an `Option` to `false` is what `is_some_and` means.
fn option_false_default(width: Option<u32>) -> bool {
    width.map_or(false, |width| width > 2)
}

// Bad: defaulting an `Option` to `true` is what `is_none_or` means.
fn option_true_default(width: Option<u32>) -> bool {
    width.map_or(true, |width| width > 2)
}

// Bad: defaulting a `Result` to `false` is what `is_ok_and` means.
fn result_false_default(width: Result<u32, String>) -> bool {
    width.map_or(false, |width| width > 2)
}

// Bad: `Result` has no counterpart of `is_none_or`, so a `true`
// default takes the chain.
fn result_true_default(width: Result<u32, String>) -> bool {
    width.map_or(true, |width| width > 2)
}

// Bad: defaulting an `Option` to `None` is what `and_then` means.
fn option_none_default(width: Option<u32>) -> Option<u32> {
    width.map_or(None, |width| width.checked_add(1))
}

// Bad: reaching `and_then` from a `Result` would take an `ok()`
// first, so this default takes the chain.
fn result_none_default(width: Result<u32, String>) -> Option<u32> {
    width.map_or(None, |width| width.checked_add(1))
}

// Bad: a negated numeric literal is a literal, and the exemption was
// the only thing keeping it quiet.
fn negated_literal_default(offset: Option<i32>) -> i32 {
    offset.map_or(-1, |offset| offset + 1)
}

// Bad: a binding named on its own is a path.
fn identifier_default(width: Option<u32>, fallback: u32) -> u32 {
    width.map_or(fallback, |width| width + 1)
}

fn main() {}
