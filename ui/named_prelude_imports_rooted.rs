// aux-build:rooted_prelude.rs
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

// A cherry-pick written with a leading `::` names the extern crate
// whatever else is in scope, so the rewrite has to keep that `::`.
// Without it the first segment goes back to ordinary resolution, where
// the module below of the same name captures it and the rewritten
// import no longer reaches the crate the author named — and this is the
// shape where that matters most, since the rewrite is offered as one
// the fixer may apply unattended.
extern crate rooted_prelude;

mod shadowed {
    pub mod rooted_prelude {
        pub struct Other;
    }

    use ::rooted_prelude::prelude::Item;
}

fn main() {}
