// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

extern crate alloc;

// Good under `also_alloc = false`: the `alloc` half of the rule is off,
// so a crate that keeps its `alloc::` paths on purpose is left alone.
mod alloc_path {
    use alloc::sync::Arc;
}

// Bad: the `core` half is unaffected by the knob.
mod core_path {
    use core::fmt::Display;
}

fn main() {}
