#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "single_block"`: every `use` sits in one contiguous
// block. A blank line between imports is the violation; the fix
// re-renders the run in source order with no blank lines.

mod clap {
    pub struct Parser;
}
mod config {
    pub struct Args;
}

// Bad: blank lines split what must be one contiguous block.
use std::time::Duration;

use clap::Parser;
use crate::config::Args;

struct Sep1;

// Good: already one contiguous block, no blank lines.
use std::io::stdin;
use std::cmp::Ordering;

fn main() {}
