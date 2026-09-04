#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = "fixture")]

// Control: a crate-root violation that is NOT under the `#[allow]`, so
// the rule must still flag it. Without it the `#[allow]` assertion would
// pass trivially even if separate-file submodules were skipped entirely.
use std::collections::HashMap;

pub use std::path::Path;

#[allow(
    perfectionist::arbitrary_source_item_ordering,
    reason = "regression fixture"
)]
mod separate;
