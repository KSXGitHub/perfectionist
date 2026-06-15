// aux-build:my_std.rs
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// With `std_crates = ["std", "core", "alloc", "my_std"]`, imports rooted
// at `my_std` join the std group. `my_std` is a real auxiliary crate, so
// without the extension it would be third-party, and a std block followed
// by a blank line and a third-party block is compliant — so this fixture
// only fires because of the knob.

extern crate my_std;

// Bad: `my_std` is configured as std, so it belongs in the std group with
// no blank line separating it from the real std import.
use std::time::Duration;

use my_std::Helper;

fn main() {}
