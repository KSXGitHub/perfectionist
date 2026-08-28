#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "single_block"` with `reexports = "split"`: submodule
// re-exports and alias re-exports form two leading blocks, every private
// import collapses into one block below, and a `#[cfg(...)]`-gated private
// import still forms a trailing block. A cfg-gated alias re-export stays
// in the alias sub-block — visibility/kind outranks cfg gating. Each run
// is isolated by a marker item, and the submodules and aliases use
// distinct names so the file still compiles.

mod iter {
    pub struct Iter;
}
mod reflection {
    pub struct Reflection;
}
mod node {
    pub struct Node;
}
mod data {
    pub struct Data;
}

// Bad: an alias re-export above the submodule re-exports, with private
// imports interleaved instead of collapsed into one block below.
pub use Reflection as HardlinkListReflection;
use std::time::Duration;
pub use iter::Iter;
pub use reflection::Reflection;
use std::io::stdin;

struct Sep1;

// Bad: all four blocks present but interleaved — a submodule re-export, an
// alias re-export, the single private block, and a trailing cfg private
// import. The cfg-gated alias re-export stays in the alias block.
use std::cmp::Ordering;
#[cfg(unix)]
pub use Node as UnixNode;
pub use node::Node;
#[cfg(unix)]
use std::collections::BTreeMap;

struct Sep2;

// Good: the submodule re-export leads, then the alias re-export, then one
// contiguous block of private imports, each region blank-separated.
pub use data::Data;

pub use Data as SharedData;

use std::sync::Arc;
use std::path::Path;

fn main() {}
