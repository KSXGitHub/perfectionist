//! Locating string literals in a macro invocation's token stream.
//!
//! Which shape a rule wants follows from what it may safely rewrite:
//!
//! - [`find_template_literal`] returns the single *format template* —
//!   the first argument that is, on its own, a lone cooked string
//!   literal. A rule whose rewrite is output-preserving only on a
//!   genuine format template (an `\n`-fold, say) may touch nothing
//!   else.
//! - [`find_all_cooked_str_literals`] returns *every* cooked string
//!   literal anywhere in the token stream, descending into delimited
//!   groups. A rule whose rewrite is value-preserving for any literal
//!   has no reason to single out the template, and needs this one to
//!   reach literals that format-args lowering would otherwise hide
//!   from the late pass.

use crate::attr_tokens::{split_top_level_commas, token_literal};
use rustc_ast::token::LitKind;
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_span::Span;

/// Span of the first top-level argument that is, on its own, a single
/// cooked string literal — the format template. Returns `None` when no
/// such argument exists (a runtime-expression template, a `concat!`
/// result, or a template that is the second argument behind a writer
/// expression that itself isn't a lone string literal, etc.).
///
/// "First lone cooked string literal" is what makes the same scan work
/// across the whole `format!`-family: `println!`'s template is the first
/// argument, `write!`'s is the second (the writer comes first and isn't
/// a bare literal), `log!`'s is the second (the level comes first), and
/// `log::info!`'s is the first.
///
/// Raw strings (`r"..."`) are deliberately not matched: a caller
/// either folds escapes the raw form has none of, or rewrites *into*
/// the raw form, so an already-raw literal is never a candidate.
pub(crate) fn find_template_literal(tokens: &TokenStream) -> Option<Span> {
    split_top_level_commas(tokens)
        .into_iter()
        .find_map(|argument| match argument.as_slice() {
            [tree] => cooked_str_literal_span(tree),
            _ => None,
        })
}

fn cooked_str_literal_span(tree: &TokenTree) -> Option<Span> {
    let (literal, span) = token_literal(tree)?;
    // Cooked (`"..."`) only. A raw string (`r"..."`) treats `\` as an
    // ordinary character, so neither the escape-aware fold in
    // `overly_long_print_macro` nor the escape-elimination scan in
    // `avoidable_string_escapes` may run over one.
    matches!(literal.kind, LitKind::Str).then_some(span)
}

/// Span of every cooked string literal anywhere in `tokens`, in source
/// order, descending into delimited groups so a literal nested inside a
/// sub-group is found too — e.g. a `maud::html!` markup string buried in
/// `code { "..." }`, which a top-level-only scan (as `overly_long_print_macro`
/// uses) would miss. The cost is that a literal inside a *nested macro
/// call* is visited once here and again when that inner macro's own
/// `check_mac` fires; `avoidable_string_escapes`'s byte-range dedup discards the
/// duplicate, so the descent is correct, just not minimal. Raw strings
/// are skipped, as in [`cooked_str_literal_span`].
pub(crate) fn find_all_cooked_str_literals(tokens: &TokenStream) -> Vec<Span> {
    let mut spans = Vec::new();
    collect_cooked_str_literals(tokens, &mut spans);
    spans
}

fn collect_cooked_str_literals(tokens: &TokenStream, spans: &mut Vec<Span>) {
    for tree in tokens.iter() {
        if let Some(span) = cooked_str_literal_span(tree) {
            spans.push(span);
        } else if let TokenTree::Delimited(_, _, _, inner) = tree {
            collect_cooked_str_literals(inner, spans);
        }
    }
}
