#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Style is the default `module`, but `respect_visibility = false`, so a
// `pub use` and a private `use` from the same module fall into one
// group. Merging them can't preserve both visibilities, so the lint
// fires but withholds an autofix.

mod m {
    pub use std::collections::BTreeMap;
    use std::collections::HashMap;
}

fn main() {}
