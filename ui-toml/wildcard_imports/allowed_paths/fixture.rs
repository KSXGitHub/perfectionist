#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

mod secret {
    pub mod internals {
        pub struct X;
    }
}

mod other {
    pub struct Y;
}

// Not flagged: `crate::secret::internals` is listed in `allowed_paths`,
// even though both exceptions are disabled.
use crate::secret::internals::*;

// Flagged: not on the allow list.
use crate::other::*;

fn main() {}
