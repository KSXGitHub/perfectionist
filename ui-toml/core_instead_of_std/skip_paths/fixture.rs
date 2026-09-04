#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

// Good: exempted by `skip_paths`.
mod exempted_path {
    use core::mem::transmute;
}

// Bad, with a `help` instead of a rewrite: `size_of` is reachable
// through `std`, but rewriting the `core` token it shares with the
// exempted `transmute` would move that one too.
mod exempted_sibling {
    use core::mem::{size_of, transmute};
}

// Bad: an unexempted path is flagged as usual.
mod unexempted_path {
    use core::fmt::Display;
}

fn main() {}
