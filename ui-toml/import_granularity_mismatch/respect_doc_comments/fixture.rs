#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Style is the default `module`, but `respect_doc_comments = false`, so
// a doc-commented `use` is allowed to merge with its plain neighbour
// from the same module. The merged statement inherits the first
// statement's doc comment, so the fix is only `MaybeIncorrect`.

mod m {
    /// Keep the two collection imports together.
    use std::collections::HashMap;
    use std::collections::BTreeMap;
}

fn main() {}
