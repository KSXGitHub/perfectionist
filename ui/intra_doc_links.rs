//! UI sweep for `intra_doc_links` under the default configuration.
//!
//! The crate-level mention of `Helper` resolves at the crate root, so
//! this very line is flagged too.
#![allow(dead_code, reason = "ui fixture")]

pub struct Helper;

pub struct Store;

/// Installs using `Helper` and `Store`.
pub fn uses_both() {}

/// Already linked [`Helper`] and [`Store`](crate::Store) stay put.
pub fn already_linked() {}

/// An unknown `Nonexistent` word names nothing in scope.
pub fn unknown_word() {}

/// A path-shaped `crate::Store` or call-shaped `uses_both()` span is
/// left alone; only bare single identifiers are candidates.
pub fn not_a_plain_ident() {}

/// Code blocks are skipped:
///
/// ```
/// let helper = "`Helper`";
/// ```
pub fn in_code_block() {}

mod inner {
    use super::Helper;

    /// Reaches `Helper` through the `use` import in this module.
    pub fn via_import() {}

    /// But `Store` is not in scope inside `inner`, so it is left alone.
    pub fn store_not_in_scope() {}
}

/// A module and a function both answer to `overlap`, which therefore
/// resolves in two namespaces and earns only a help note.
pub fn refer_overlap() {}

mod overlap {}

fn overlap() {}

fn main() {}
