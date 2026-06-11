#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    ambiguous_glob_reexports,
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

// Not flagged: `::crate::secret::internals` is listed in `allowed_paths`,
// even though both exceptions are disabled.
use crate::secret::internals::*;

// Both globs from `::std::collections` are exempt under the single
// `allowed_paths` entry `"::std::collections"`, whether or not the `use`
// writes the leading `::` — the leading `::` in the config is the
// absolute marker and covers both written forms. (`::`-rooting only
// applies to extern crates, so `std` carries this case, not the
// crate-local `secret` above.)
use std::collections::*;
use ::std::collections::*;

// Flagged: not on the allow list.
use crate::other::*;

fn main() {}
