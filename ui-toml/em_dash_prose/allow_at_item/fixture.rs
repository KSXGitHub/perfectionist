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

#[cfg_attr(
    dylint_lib = "perfectionist",
    allow(
        perfectionist::em_dash_prose,
        reason = "quotes the dash on purpose"
    )
)]
/// Allowed here, so this em dash — is not flagged.
pub fn allowed() {}

/// But this em dash — is still flagged.
pub fn flagged() {}

fn main() {}
