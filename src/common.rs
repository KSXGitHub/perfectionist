//! Helpers shared between sibling rules.
//!
//! Each helper lives here only because more than one rule needs it.
//! Anything used by a single rule belongs in that rule's own file.

use std::collections::BTreeSet;

use rustc_hir as hir;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{Span, Symbol};

/// Whether the HIR node at `hir_id` (whose own span is `span`)
/// originates in an external proc-macro (or `macro_rules!`)
/// expansion.
///
/// `declare_tool_lint!(... report_in_external_macro: false)` only
/// inspects the diagnostic span when deciding whether to suppress.
/// Proc-macro derives such as `clap_derive`'s `default_value_t`
/// expansion synthesise nodes whose identifier inherits a
/// user-source span (the span of the attribute that drove the
/// expansion) so that downstream compile errors point somewhere a
/// user can fix; from the lint's perspective the identifier looks
/// user-authored even though the surrounding statement only exists
/// in the expansion. Every rule whose diagnostic span is narrower
/// than the syntactic node that produced the violation must
/// therefore check the structural-parent span explicitly.
///
/// Two checks are needed because some structural spans cover only
/// the identifier itself (a `<T>` generic parameter has no other
/// tokens), so the node's own `Span::in_external_macro` returns
/// false. Walking up to the enclosing item and checking its
/// `def_span` catches that case — the synthesised owner item's
/// span carries the expansion's `SyntaxContext`. Regression
/// fixtures live in `ui/*_proc_macro.rs` with a minimal derive in
/// `ui/auxiliary/proc_macro_synth_binding.rs`.
pub(crate) fn hir_in_external_macro(cx: &LateContext<'_>, hir_id: HirId, span: Span) -> bool {
    let sm = cx.sess().source_map();
    if span.in_external_macro(sm) {
        return true;
    }
    let owner_id = cx.tcx.hir_get_parent_item(hir_id);
    cx.tcx.def_span(owner_id.to_def_id()).in_external_macro(sm)
}

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
/// `ignore`. Used by rules whose runtime allowlist key remains
/// a `String` (currently just `non_exhaustive_error`, whose
/// suffix lookup is `str::ends_with`-shaped); the four rules
/// whose late-pass lookup key is a [`Symbol`] use the sibling
/// [`merge_symbol_allowlist`] instead. The `BTreeSet` return is
/// convenient for set membership lookups and has the side
/// benefit of dropping duplicates when defaults and extras
/// overlap; callers that need a `Vec`-shaped result can
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
