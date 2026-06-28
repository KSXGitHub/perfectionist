// aux-build:clap.rs
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `order = ["std", "thirdparty", "internal"]`: third-party
// crates group ahead of internal (`crate` / `super` / `self`) imports.
// `clap` is a real auxiliary crate (genuinely third-party); `config` is
// a first-party module reached through `crate::`.

extern crate clap;

mod config {
    pub struct Args;
    pub struct Bytes;
}

// Bad: internal group appears before third-party, which the custom
// order reverses.
use std::time::Duration;

use crate::config::Args;

use clap::Parser;

struct Sep1;

// Good: std, third-party, internal — matching the custom order.
use std::io::stdin;

use clap::Subcommand;

use crate::config::Bytes;

fn main() {}
