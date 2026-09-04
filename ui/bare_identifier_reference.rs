// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
//! UI sweep for `bare_identifier_reference` under the default configuration.
//!
//! The crate-level mention of `Helper` resolves at the crate root, so
//! this very line is flagged too.
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    dead_code,
    perfectionist::arbitrary_source_item_ordering,
    reason = "ui fixture"
)]

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

pub mod overlap {}

pub fn overlap() {}

struct PrivateHelper;

/// A `pub` function that mentions the private `PrivateHelper` is left
/// alone: linking a public item to a private one would trip rustdoc's
/// own `private_intra_doc_links`.
pub fn public_refers_private() {}

/// A private function, by contrast, may link the private
/// `PrivateHelper` — both sit below the public API — so this one fires.
fn private_refers_private() {}

/// A public module and a *private* function both answer to `mixed_vis`.
/// rustdoc resolves `[`mixed_vis`]` across both namespaces (privacy is a
/// separate concern), so this is ambiguous — a help note, never a
/// machine-applicable fix — even though the private function alone would
/// be exempt.
pub fn refer_mixed_visibility() {}

pub mod mixed_vis {}

fn mixed_vis() {}

/// The inverse of `mixed_vis`: the type (`flipped_vis`) is private and
/// the public function is the eligible target, so the disambiguator must
/// be `value@`, not `type@`.
pub fn refer_flipped_visibility() {}

mod flipped_vis {}

pub fn flipped_vis() {}

fn main() {}
