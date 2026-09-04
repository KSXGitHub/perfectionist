// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Under `style = "multi_block"` with `reexports = "by_path"` (the
// non-default): a `pub use` re-export is classified purely by its path,
// not pulled into a leading re-export block. A `pub use crate::...` is
// therefore grouped with the private `use crate::...` internal imports,
// and must be blank-separated from std like any other internal import.

mod config {
    pub struct Args;
    pub struct Opts;
}

// Bad: a `pub use crate::...` re-export is classified internal (by path),
// so it belongs in the same block as the private `use crate::...` import;
// here a std import is interleaved between them.
pub use crate::config::Args;
use std::time::Duration;
use crate::config::Opts;

fn main() {}
