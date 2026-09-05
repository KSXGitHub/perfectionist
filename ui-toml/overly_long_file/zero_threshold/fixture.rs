// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_lines = 0` the file is flagged and the diagnostic states
// its count: the three attribute lines above, the items below, and no
// comment or blank line.

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
