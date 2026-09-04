// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "multi_block"` with `reexports = "split"`: the leading
// re-export region splits into two blank-separated blocks — submodule
// re-exports (a `::`-qualified path) above alias re-exports (a
// single-segment path) — both above the path-partitioned private imports.
// Each run is isolated by a marker item, and the submodules and aliases
// use distinct names so the file still compiles.

mod iter {
    pub struct Iter;
}
mod reflection {
    pub struct Reflection;
}
mod summary {
    pub struct Summary;
}
mod node {
    pub struct Node;
}
mod data {
    pub struct Data;
}

// Bad: an alias re-export sits above the submodule re-exports it renames.
pub use Reflection as HardlinkListReflection;
pub use iter::Iter;
pub use reflection::Reflection;

struct Sep1;

// Bad: a submodule re-export, an alias re-export, and a private std import
// all intermixed; the fix orders them into three blocks.
pub use Summary as SharedLinkSummary;
use std::time::Duration;
pub use summary::Summary;

struct Sep2;

// Bad: a cfg-gated alias re-export stays in the alias sub-block rather
// than a trailing cfg block — visibility/kind outranks cfg gating.
use std::mem::swap;
#[cfg(unix)]
pub use Node as UnixNode;
pub use node::Node;

struct Sep3;

// Good: the submodule re-export leads, then the alias re-export, then the
// private std import, each blank-separated.
pub use data::Data;

pub use Data as SharedData;

use std::sync::Arc;

fn main() {}
