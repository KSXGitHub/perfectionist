// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Not flagged: this file has 5 lines of code, exactly the limit of 5 —
// the 3 attribute lines above and the 2 below. A file is flagged only
// above the limit, never at it.

fn work() {}

fn main() {}
