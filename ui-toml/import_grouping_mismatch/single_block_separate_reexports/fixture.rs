#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "single_block"` with `reexports = "grouped"`: `pub`
// re-exports form one leading block, the private imports collapse into a
// single block below, and a `#[cfg(...)]`-gated private import still
// forms a trailing block. Each run is isolated by a marker item and every
// import binds a distinct name so the file still compiles.

mod config {
    pub struct Args;
    pub struct Opts;
    pub struct Cfg;
}

// Bad: a `pub use` re-export sits among the private single-block imports
// instead of leading in its own block.
use std::time::Duration;
pub use crate::config::Args;
use std::io::stdin;

struct Sep1;

// Bad: all three blocks present — a leading re-export block, the single
// private block, and a trailing cfg block — but interleaved here.
use std::cmp::Ordering;
pub use crate::config::Opts;
#[cfg(unix)]
use std::collections::BTreeMap;

struct Sep2;

// Good: re-export block leads, blank-separated from one contiguous block
// of private imports.
pub use crate::config::Cfg;

use std::sync::Arc;
use std::path::Path;

fn main() {}
