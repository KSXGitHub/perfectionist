//! Helpers shared between sibling rules.
//!
//! Each helper lives here only because more than one rule needs it.
//! Anything used by a single rule belongs in that rule's own file.

use rustc_hir as hir;

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
