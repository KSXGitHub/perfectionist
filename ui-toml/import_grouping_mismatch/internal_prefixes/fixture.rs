#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `internal_prefixes = ["crate", "super", "self", "my_macros"]`:
// imports rooted at `my_macros` are treated as internal, grouping with
// `crate` / `super` / `self` ahead of third-party crates.

mod clap {
    pub struct Parser;
    pub struct Subcommand;
}
mod my_macros {
    pub struct Helper;
    pub struct Util;
}

// Bad: `my_macros` (internal) appears after `clap` (third-party);
// internal must come before third-party.
use std::time::Duration;

use clap::Parser;

use my_macros::Helper;

struct Sep1;

// Good: std, internal (`my_macros`), then third-party.
use std::io::stdin;

use my_macros::Util;

use clap::Subcommand;

fn main() {}
