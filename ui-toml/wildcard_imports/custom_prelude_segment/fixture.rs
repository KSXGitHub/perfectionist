#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

mod foo {
    pub mod api {
        pub struct A;
    }
    pub mod prelude {
        pub struct B;
    }
}

// Not flagged: `api` is now a recognised prelude segment name.
use crate::foo::api::*;

// Flagged: `prelude` is no longer in `prelude_segment_names`.
use crate::foo::prelude::*;

fn main() {}
