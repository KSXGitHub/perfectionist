#![feature(register_tool)]
#![register_tool(perfectionist)]
#![no_std]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

// A `#![no_std]` crate has no `std::` to name, so nothing here is
// flagged — not even with `std` linked into the same compilation, which
// is what a `#[cfg(test)] extern crate std;` amounts to.
extern crate std;

mod core_path {
    use core::fmt::Display;
}

fn main() {}
