// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Bad: this file has 10 lines of code, above the limit of 0 — the 3
// attribute lines above and the items below; its comments and blank
// lines are free.

/// A documented function; the doc comment is not code.
fn work(value: u32) -> u32 {
    value
}

/* A block comment
   spanning lines. */
mod inline {
    // An inline module belongs to this file.
    pub fn inner() {}
}

fn main() {}
