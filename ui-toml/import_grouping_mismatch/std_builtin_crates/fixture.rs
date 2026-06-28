#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// The std group is the fixed set `std` / `core` / `alloc` / `proc_macro`
// / `test`: `proc_macro` and `test` are compiler-provided crates grouped
// with std, not treated as third-party.
//
// Stand-in modules so every import resolves; classification keys on the
// path's first segment, so a local `mod proc_macro` stands in for the
// real crate. Each run is separated by a marker item and binds a distinct
// name so the crate-root file still compiles.

mod proc_macro {
    pub struct TokenStream;
}
mod test {
    pub struct Bencher;
}
mod clap {
    pub struct Parser;
}

// Bad: a blank line splits the std group — `proc_macro` and `test` join
// `std`, so the three belong in one contiguous block with no blank.
use std::time::Duration;

use proc_macro::TokenStream;
use test::Bencher;

struct Sep1;

// Good: the std group (`std` + `proc_macro`) is one contiguous block,
// blank-separated from the third-party import below it.
use std::io::stdin;
use proc_macro::TokenStream as Tokens;

use clap::Parser;

fn main() {}
