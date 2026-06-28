// aux-build:clap.rs
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Regression: a bare-path import of a first-party submodule (`use
// error::Foo;`, where `error` is a sibling `mod`) is classified as
// internal — grouped with `crate::` imports, ahead of third-party — not
// as a third-party crate keyed on the bare first segment. `clap` is a
// real auxiliary crate (genuinely third-party) for contrast.

extern crate clap;

mod error {
    pub struct Foo;
}

// Bad: third-party (`clap`) sits before the bare-path internal submodule
// (`error`), reversing the std / internal / third-party order. The fix
// reorders to put `error` (internal) ahead of `clap` (third-party).
use std::time::Duration;

use clap::Parser;

use error::Foo;

fn main() {}
