//! Locating string literals in a macro invocation's token stream.
//!
//! Which shape a rule wants follows from what it may safely rewrite:
//!
//! - [`find_template_literal`] returns the single *format template*,
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
//! - [`find_message_literals`] returns every top-level argument past
//!   a given count that is a lone string literal, raw and cooked forms
//!   alike. A rule that only reads a literal's prose has no escape to
//!   fold or to eliminate, so the raw spelling is as readable as the
//!   cooked one, and every such argument of a `println!`-family or
//!   `panic!`-family call is text the program shows its user.

use crate::attr_tokens::{split_top_level_commas, token_literal};
use rustc_ast::token::LitKind;
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_span::Span;

/// Span of the first top-level argument that is, on its own, a single
/// cooked string literal, the format template. Returns `None` when no
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

/// Span of every top-level argument past the first `skip` that is, on
/// its own, a single string literal, raw and cooked spellings alike,
/// in source order.
///
/// `skip` is how many leading comma-separated arguments hold something
/// other than a message: `assert!`'s condition is one, `assert_eq!`'s
/// pair of compared values is two, and a macro whose first argument is
/// already the message skips none. Without it a compared value would
/// be read as a message, which is program data rather than prose.
///
/// Lone and top-level are what keep the remaining non-message
/// arguments out of the result: `write!`'s writer and `log!`'s level
/// are not literals at all, a `target: "..."` pair is three token
/// trees rather than one, and a literal nested inside another macro
/// call belongs to that call, which gets its own visit. A caller that
/// wants only the format template, and only in the cooked spelling it
/// can rewrite escapes in, wants [`find_template_literal`] instead.
pub(crate) fn find_message_literals(tokens: &TokenStream, skip: usize) -> Vec<Span> {
    split_top_level_commas(tokens)
        .into_iter()
        .skip(skip)
        .filter_map(|argument| match argument.as_slice() {
            [tree] => str_literal_span(tree),
            _ => None,
        })
        .collect()
}

/// Span of a string literal token tree, raw and cooked spellings
/// alike, or `None` for anything else.
fn str_literal_span(tree: &TokenTree) -> Option<Span> {
    let (literal, span) = token_literal(tree)?;
    matches!(literal.kind, LitKind::Str | LitKind::StrRaw(_)).then_some(span)
}

/// Span of every cooked string literal anywhere in `tokens`, in source
/// order, descending into delimited groups so a literal nested inside a
/// sub-group is found too, say a `maud::html!` markup string buried in
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
