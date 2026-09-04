// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    ambiguous_glob_reexports,
    hidden_glob_reexports,
    reason = "ui fixture"
)]

mod thing {
    pub struct A;
    pub struct B;
}

mod prelude {
    pub use crate::thing::{A, B};
}

// Flagged now: the prelude exception is disabled.
use crate::prelude::*;

// Flagged now: the root-re-export exception is disabled.
pub use crate::thing::*;

// Flagged: a plain glob, as always.
use crate::thing::*;

fn main() {}
