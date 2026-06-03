//! Locating the format template in a macro invocation's token stream.
//!
//! A `format!`-family macro takes its format string as the first
//! argument that is, on its own, a single cooked string literal. The
//! same "first lone cooked string literal" rule pins the template down
//! across the whole family regardless of which positional slot it lands
//! in: `format!`'s and `println!`'s template is the first argument,
//! `write!`'s is the second (the writer comes first and isn't a bare
//! literal), `log!`'s is the second (the level comes first), and
//! `log::info!`'s is the first.
//!
//! Two rules read this: `print_macro_split`, which folds a long
//! template across lines, and `prefer_raw_string`, which rewrites a
//! template whose only escapes are raw-expressible into the raw-string
//! form even when format-args lowering would otherwise hide it from the
//! late pass.

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_span::Span;

/// Span of the first top-level argument that is, on its own, a single
/// cooked string literal — the format template. Returns `None` when no
/// such argument exists (a runtime-expression template, a `concat!`
/// result, or a template that is the second argument behind a writer
/// expression that itself isn't a lone string literal, etc.).
///
/// Raw strings (`r"..."`) are deliberately not matched: both callers
/// either fold escapes the raw form has none of, or rewrite *into* the
/// raw form, so a literal that is already raw is never a candidate.
pub(crate) fn find_template_literal(tokens: &TokenStream) -> Option<Span> {
    let mut argument_len: usize = 0;
    let mut argument_lead_literal: Option<Span> = None;
    let mut found: Option<Span> = None;
    let finish_argument = |len: usize, lead: Option<Span>, found: &mut Option<Span>| {
        if found.is_none() && len == 1 {
            *found = lead;
        }
    };
    for tree in tokens.iter() {
        if is_top_level_comma(tree) {
            finish_argument(argument_len, argument_lead_literal, &mut found);
            argument_len = 0;
            argument_lead_literal = None;
            continue;
        }
        if argument_len == 0 {
            argument_lead_literal = cooked_str_literal_span(tree);
        }
        argument_len += 1;
    }
    finish_argument(argument_len, argument_lead_literal, &mut found);
    found
}

fn is_top_level_comma(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(token, _) if token.kind == TokenKind::Comma)
}

fn cooked_str_literal_span(tree: &TokenTree) -> Option<Span> {
    let TokenTree::Token(token, _) = tree else {
        return None;
    };
    let TokenKind::Literal(literal) = token.kind else {
        return None;
    };
    // Cooked (`"..."`) only. A raw string (`r"..."`) treats `\` as an
    // ordinary character, so neither the escape-aware fold in
    // `print_macro_split` nor the escape-elimination scan in
    // `prefer_raw_string` may run over one.
    matches!(literal.kind, LitKind::Str).then_some(token.span)
}
