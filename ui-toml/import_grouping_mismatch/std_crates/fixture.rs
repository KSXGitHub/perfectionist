#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// With `std_crates = ["std", "core", "alloc", "my_std"]`, imports rooted
// at `my_std` join the std group. Without the extension `my_std` would be
// third-party, and a std block followed by a blank line and a third-party
// block is compliant — so this fixture only fires because of the knob.

mod my_std {
    pub struct Helper;
}

// Bad: `my_std` is configured as std, so it belongs in the std group with
// no blank line separating it from the real std import.
use std::time::Duration;

use my_std::Helper;

fn main() {}
