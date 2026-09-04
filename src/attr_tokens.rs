//! Reading `#[name(...)]` attributes and their argument tokens off the
//! AST.
//!
//! Everything here is read from tokens rather than from
//! `meta_item_list`, so it works on a freshly re-parsed module AST (see
//! [`crate::module_reparse`]) and on argument forms that are not meta
//! items — `#[display("{}", self.0)]` has an expression where a meta
//! item would have to be. Nothing here knows about any particular
//! attribute: [`attribute_calls`] unwraps a `#[name(...)]` (looking
//! through `#[cfg_attr(...)]`), [`is_cfg_gated`] answers whether a
//! `#[cfg(...)]` gates a node, and [`split_top_level_commas`] /
//! [`ident_name`] / [`token_literal`] / [`str_literal`] read the
//! argument tokens.
//!
//! The token readers are useful beyond attributes: a macro call's
//! arguments are the same comma-separated token stream, which is what
//! `crate::macro_template` and `perfectionist::impure_macro_arguments`
//! split.

use rustc_ast::token::{Lit, TokenKind};
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_ast::{AttrArgs, AttrKind, Attribute, LitKind};
use rustc_span::{Span, Symbol, sym};

/// One `#[name(...)]` attribute as written on a node, with any
/// `#[cfg_attr(<cfg>, ...)]` wrapper peeled off.
pub(crate) struct AttributeCall<'a> {
    /// Final segment of the attribute's path. Taking the last segment
    /// matches the attribute whether it is written qualified
    /// (`derive_more::display`) or not.
    pub(crate) name: Symbol,
    /// The attribute's argument tokens, without the delimiters.
    pub(crate) tokens: &'a TokenStream,
    /// Whether a `#[cfg_attr(...)]` gates the attribute.
    pub(crate) gated: bool,
    /// The whole attribute. For a gated call this is the enclosing
    /// `#[cfg_attr(...)]`, since the peeled-off attribute has no span of
    /// its own.
    pub(crate) span: Span,
}

/// Every `#[name(...)]` attribute `attr` carries, looking through a
/// `#[cfg_attr(<cfg>, ...)]` gate — and through a nested one, since
/// `cfg_attr(a, cfg_attr(b, ...))` is how `cfg_attr(all(a, b), ...)` is
/// sometimes spelled. An attribute in any other shape (`#[foo]`,
/// `#[foo = "bar"]`, a doc comment) yields nothing.
pub(crate) fn attribute_calls(attr: &Attribute) -> Vec<AttributeCall<'_>> {
    let AttrKind::Normal(_) = &attr.kind else {
        return Vec::new();
    };
    let item = attr.get_normal_item();
    let Some(name) = item.path.segments.last().map(|segment| segment.ident.name) else {
        return Vec::new();
    };
    let Some(AttrArgs::Delimited(args)) = item.args.unparsed_ref() else {
        return Vec::new();
    };
    if name == sym::cfg_attr {
        return gated_calls(&args.tokens, attr.span);
    }
    vec![AttributeCall {
        name,
        tokens: &args.tokens,
        gated: false,
        span: attr.span,
    }]
}

/// Every `#[name(...)]` attribute across `attrs`, each unwrapped through
/// any `#[cfg_attr(...)]` gate.
pub(crate) fn attribute_calls_of(attrs: &[Attribute]) -> Vec<AttributeCall<'_>> {
    attrs.iter().flat_map(attribute_calls).collect()
}

/// The attributes a `#[cfg_attr(<cfg>, ...)]` applies, given its
/// argument tokens. The first comma-separated group is the predicate
/// and is skipped; every group after it is an attribute the predicate
/// gates, and is itself unwrapped when it is another `cfg_attr`.
fn gated_calls(tokens: &TokenStream, span: Span) -> Vec<AttributeCall<'_>> {
    let mut calls = Vec::new();
    for group in split_top_level_commas(tokens).into_iter().skip(1) {
        let [name, arguments] = group.as_slice() else {
            continue;
        };
        let (Some(name), TokenTree::Delimited(_, _, _, inner)) = (ident_name(name), arguments)
        else {
            continue;
        };
        if name == sym::cfg_attr {
            calls.extend(gated_calls(inner, span));
        } else {
            calls.push(AttributeCall {
                name,
                tokens: inner,
                gated: true,
                span,
            });
        }
    }
    calls
}

/// Whether a `#[cfg(...)]` gates the node, including one applied through
/// a `#[cfg_attr(...)]`.
pub(crate) fn is_cfg_gated(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.has_name(sym::cfg)
            || attribute_calls(attr)
                .iter()
                .any(|call| call.name == sym::cfg)
    })
}

/// Split a token stream into comma-separated groups. A comma inside a
/// delimited group is not top-level, so `cfg_attr`'s `all(a, b)`
/// predicate stays one group.
pub(crate) fn split_top_level_commas(tokens: &TokenStream) -> Vec<Vec<&TokenTree>> {
    let mut groups = vec![Vec::new()];
    for tree in tokens.iter() {
        if matches!(tree, TokenTree::Token(token, _) if token.kind == TokenKind::Comma) {
            groups.push(Vec::new());
        } else {
            groups
                .last_mut()
                .expect("`groups` starts non-empty and only ever grows")
                .push(tree);
        }
    }
    groups
}

/// The name of an identifier token, or `None` for any other token tree.
pub(crate) fn ident_name(tree: &TokenTree) -> Option<Symbol> {
    let TokenTree::Token(token, _) = tree else {
        return None;
    };
    token.ident().map(|(ident, _raw)| ident.name)
}

/// The literal a token tree holds, with the token's span. Callers that
/// want the literal's *value* decode it further; one that only needs to
/// point at it — `crate::macro_template` locating a format template —
/// keeps the span.
pub(crate) fn token_literal(tree: &TokenTree) -> Option<(Lit, Span)> {
    let TokenTree::Token(token, _) = tree else {
        return None;
    };
    let TokenKind::Literal(literal) = token.kind else {
        return None;
    };
    Some((literal, token.span))
}

/// The cooked value of a string literal token, raw and escaped forms
/// alike — `r"{_0}"` and `"{_0}"` are the same string.
pub(crate) fn str_literal(tree: &TokenTree) -> Option<String> {
    let (literal, _span) = token_literal(tree)?;
    let LitKind::Str(symbol, _style) = LitKind::from_token_lit(literal).ok()? else {
        return None;
    };
    Some(symbol.as_str().to_owned())
}
