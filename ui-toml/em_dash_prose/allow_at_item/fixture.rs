// A per-item `#[allow]` suppresses only that item's finding; a sibling
// item with no attribute still fires. The level resolves at the
// comment's enclosing item rather than at the crate root, so per-site
// control works without exempting the whole crate.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
#![cfg_attr(
    dylint_lib = "perfectionist",
    allow(
        perfectionist::allow_attributes,
        reason = "fixture targets `em_dash_prose`; the per-item `allow` is the subject, not a rewrite candidate"
    )
)]

// Not flagged: the per-item `allow` suppresses this item's finding.
#[cfg_attr(
    dylint_lib = "perfectionist",
    allow(
        perfectionist::em_dash_prose,
        reason = "this item's documentation uses the dash on purpose"
    )
)]
/// Walks the tree — and returns the total size.
pub fn allowed() {}

// Bad: a sibling item carrying no attribute still fires.
/// Reads the manifest — and returns its package name.
pub fn flagged() {}

fn main() {}
