// A per-item `#[allow]` suppresses only that item's doc-comment
// finding; a sibling item with no attribute still fires. This pins the
// fix for <https://github.com/KSXGitHub/perfectionist/issues/165>: the
// level now resolves at the comment's enclosing item, not the crate
// root, so per-site control works without exempting the whole crate.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

#[cfg_attr(
    dylint_lib = "perfectionist",
    allow(
        perfectionist::unicode_ellipsis_in_docs,
        reason = "documents the ellipsis glyph on purpose"
    )
)]
/// Allowed here, so this ellipsis … is not flagged.
pub fn allowed() {}

/// But this ellipsis … is still flagged.
pub fn flagged() {}

fn main() {}
