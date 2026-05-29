#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `blank_line_count = 2`: adjacent groups are separated by
// exactly two blank lines. One blank line is a violation.

mod clap {
    pub struct Parser;
    pub struct Subcommand;
}

// Bad: std and third-party groups separated by a single blank line.
use std::time::Duration;

use clap::Parser;

struct Sep1;

// Good: the two groups are separated by two blank lines.
use std::io::stdin;


use clap::Subcommand;

fn main() {}
