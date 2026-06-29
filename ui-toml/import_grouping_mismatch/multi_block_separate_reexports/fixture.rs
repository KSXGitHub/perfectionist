#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "multi_block"` with `reexports = "grouped"`: every
// `pub` re-export forms one leading block, blank-separated from the
// path-partitioned private imports below. Visibility outranks path and
// cfg gating, so a `pub use std::...` and a cfg-gated `pub use` both join
// the leading re-export block rather than their natural path / trailing
// cfg group. Each run is isolated by a marker item and every import binds
// a distinct name so the file still compiles.

mod config {
    pub struct Args;
    pub struct Opts;
    pub struct Cfg;
    pub struct Bytes;
    pub struct More;
}

// Bad: a `pub use` re-export is interleaved with private std / internal
// imports instead of leading in its own block.
use std::time::Duration;
pub use crate::config::Args;
use crate::config::Opts;

struct Sep1;

// Bad: visibility outranks path — a `pub use std::...` re-export joins
// the leading block with a `pub use crate::...` re-export rather than the
// std group, and both must sit above the private std import.
pub use std::cmp::Ordering;
use std::fmt::Write;
pub use crate::config::Cfg;

struct Sep2;

// Bad: a cfg-gated re-export stays in the leading re-export block, not
// the trailing cfg block — visibility takes precedence over cfg gating —
// so it must move above the private std import.
use std::mem::swap;
#[cfg(unix)]
pub use std::collections::BTreeMap;

struct Sep3;

// Good: the lone re-export block leads, then std, then internal, each
// blank-separated.
pub use crate::config::Bytes;

use std::sync::Arc;

use crate::config::More;

fn main() {}
