//! Helpers shared between sibling rules.
//!
//! Each helper lives here only because more than one rule needs it.
//! Anything used by a single rule belongs in that rule's own file.

use std::collections::BTreeSet;

use rustc_hir as hir;
use rustc_span::Symbol;

/// Whether `name` is exactly one ASCII letter (`a`..=`z` or
/// `A`..=`Z`). Used by every `single_letter_*` rule.
pub(crate) fn is_single_ascii_letter(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    chars.next().is_none() && first.is_ascii_alphabetic()
}

/// Extract the identifier from a plain `Binding(_, _, ident, None)`
/// pattern. Returns `None` for any non-binding pattern or a binding
/// with a sub-pattern. Used by the `let`-binding, function-parameter,
/// and closure-parameter rules.
pub(crate) fn binding_ident<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<rustc_span::Ident> {
    match pat.kind {
        hir::PatKind::Binding(_, _, ident, None) => Some(ident),
        _ => None,
    }
}

/// Sibling of [`binding_ident`] that returns the binding's `HirId`
/// instead of its `Ident`. Used by the closure-parameter rule to test
/// whether a particular expression refers to one of the closure's
/// parameters.
pub(crate) fn binding_hir_id<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<hir::HirId> {
    match pat.kind {
        hir::PatKind::Binding(_, hir_id, _, None) => Some(hir_id),
        _ => None,
    }
}

/// Merge a curated built-in allowlist of `&str` defaults with a
/// user-supplied `extras` list, then subtract every entry in
/// `ignore`. Used by every rule whose config follows the
/// `extra_<thing>` / `ignore_<thing>` pair convention. The
/// `BTreeSet` return is convenient for set membership lookups and
/// has the side benefit of dropping duplicates when defaults and
/// extras overlap; callers that need a `Vec`-shaped result can
/// `.into_iter().collect()` it themselves.
pub(crate) fn merge_string_allowlist(
    defaults: &[&str],
    extras: Vec<String>,
    ignore: Vec<String>,
) -> BTreeSet<String> {
    let ignore: BTreeSet<String> = ignore.into_iter().collect();
    defaults
        .iter()
        .map(ToString::to_string)
        .chain(extras)
        .filter(|name| !ignore.contains(name))
        .collect()
}

/// Sibling of [`merge_string_allowlist`] that interns each name as
/// a [`Symbol`] in one pass — skipping the intermediate
/// `BTreeSet<String>` of the string-shaped variant. Used by rules
/// whose late-pass lookup key is already a `Symbol`
/// (`unicode_ellipsis_in_panic_messages`, the three `single_letter_*`
/// rules), so that membership checks reduce to integer compares
/// instead of `Symbol::as_str` → `String` round-trips.
///
/// Must be called inside a rustc session, since [`Symbol::intern`]
/// reaches into the per-session symbol table.
pub(crate) fn merge_symbol_allowlist(
    defaults: &[&str],
    extras: Vec<String>,
    ignore: Vec<String>,
) -> BTreeSet<Symbol> {
    let ignore: BTreeSet<Symbol> = ignore.iter().map(|name| Symbol::intern(name)).collect();
    defaults
        .iter()
        .map(|name| Symbol::intern(name))
        .chain(extras.iter().map(|name| Symbol::intern(name)))
        .filter(|sym| !ignore.contains(sym))
        .collect()
}
