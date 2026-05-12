//! Argument splitting and the trivial-expression predicate.
//!
//! [`split_top_level_arguments`] turns the macro invocation's
//! token stream into one segment per comma-separated argument.
//! [`looks_like_expression`] rules out non-expression positions the
//! macro author chose (`Type => [...]` and friends).
//! [`is_trivial_expression`] decides whether the surviving expression
//! falls in the spec's seven trivial shapes.
//!
//! The predicate is a hand-rolled token-stream walker — see the
//! rationale in `planned-rules/macro-argument-binding.md`'s
//! "Implementation notes" section. The walker is `take_*`-style per
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`.

use rustc_ast::token::{Delimiter, IdentIsRaw, TokenKind};
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_span::kw;

/// Split the top-level token stream of a macro invocation into one
/// segment per comma-separated argument. Returns `None` if a top-level
/// `;` is encountered (the repeat form, `vec![v; count]`), which
/// signals that the invocation is not a comma-separated argument list
/// and the rule skips the whole call.
///
/// `=>` is ordinary content here — match-arm syntax inside `matches!`
/// shows up as a top-level fat arrow but is meaningful to the macro,
/// not a separator. The walker passes it through unchanged so each
/// argument's `looks_like_expression` check can skip it as a
/// non-expression position the macro author chose.
pub(super) fn split_top_level_arguments(stream: &TokenStream) -> Option<Vec<Vec<TokenTree>>> {
    let mut arguments: Vec<Vec<TokenTree>> = Vec::new();
    let mut current: Vec<TokenTree> = Vec::new();
    for tree in stream.iter() {
        if let TokenTree::Token(token, _) = tree {
            match token.kind {
                TokenKind::Semi => return None,
                TokenKind::Comma => {
                    arguments.push(std::mem::take(&mut current));
                    continue;
                }
                _ => {}
            }
        }
        current.push(tree.clone());
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    Some(arguments)
}

/// Heuristic: does the argument plausibly parse as a single Rust
/// expression? The rule docs say "skip arguments that don't parse as a
/// single expression (`name: type`, `name = value`, etc. are syntactic
/// positions the macro author chose)" and prescribe a `Parser::parse_expr`
/// re-parse to make that call. We approximate without `rustc_parse` to
/// avoid emitting parser-recovery diagnostics for arbitrary macro
/// inputs: a top-level `=>` token is a match-arm separator (`matches!`,
/// `impl_lint_pass!`-style `Type => [LINT_NAMES]` DSLs) and is never
/// part of a single Rust expression. Other non-expression markers like
/// `name: type` and `name = value` are not reliably distinguishable
/// from valid expression syntax (`expr: type` ascription, assignment),
/// and a future re-parse-based implementation will subsume this check.
pub(super) fn looks_like_expression(argument: &[TokenTree]) -> bool {
    !argument.iter().any(|tree| {
        matches!(
            tree,
            TokenTree::Token(token, _) if token.kind == TokenKind::FatArrow,
        )
    })
}

/// Returns `true` if the entire token slice forms a "trivial"
/// expression per the rule's grammar. Triviality is purely syntactic:
/// the seven shapes the rule docs enumerate, recursive on operands.
/// Anything outside that grammar is non-trivial — including `const fn`
/// calls and other "morally pure" expressions.
pub(super) fn is_trivial_expression(tokens: &[TokenTree]) -> bool {
    take_trivial_expression(tokens).is_some_and(<[_]>::is_empty)
}

fn take_trivial_expression(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let after_atom = take_trivial_atom(tokens)?;
    Some(take_trivial_suffixes(after_atom))
}

fn take_trivial_atom(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = head else {
        return None;
    };
    match token.kind {
        TokenKind::Literal(_) => Some(rest),
        // `true` and `false` are keyword idents, not `Literal` tokens.
        TokenKind::Ident(name, IdentIsRaw::No) if name == kw::True || name == kw::False => {
            Some(rest)
        }
        // `&` expr or `&mut` expr.
        TokenKind::And => take_reference_tail(rest),
        // `&&` expr or `&& mut` expr (double reference).
        TokenKind::AndAnd => take_reference_tail(rest),
        // `*expr` (deref).
        TokenKind::Star => take_trivial_expression(rest),
        // Path: ident (`::` ident)*.
        TokenKind::Ident(_, _) => Some(take_path_tail(rest)),
        // Leading `::` — must be followed by an ident.
        TokenKind::PathSep => take_path_after_sep(rest),
        _ => None,
    }
}

fn take_reference_tail(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let after_mut = match tokens.split_first() {
        Some((TokenTree::Token(token, _), rest)) if token.is_keyword(kw::Mut) => rest,
        _ => tokens,
    };
    take_trivial_expression(after_mut)
}

fn take_path_tail(mut tokens: &[TokenTree]) -> &[TokenTree] {
    while let Some((TokenTree::Token(sep, _), after_sep)) = tokens.split_first() {
        if sep.kind != TokenKind::PathSep {
            break;
        }
        let Some((TokenTree::Token(ident, _), after_ident)) = after_sep.split_first() else {
            break;
        };
        if !matches!(ident.kind, TokenKind::Ident(_, _)) {
            break;
        }
        tokens = after_ident;
    }
    tokens
}

fn take_path_after_sep(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (ident, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = ident else {
        return None;
    };
    if !matches!(token.kind, TokenKind::Ident(_, _)) {
        return None;
    }
    Some(take_path_tail(rest))
}

fn take_trivial_suffixes(mut tokens: &[TokenTree]) -> &[TokenTree] {
    loop {
        let Some((head, rest)) = tokens.split_first() else {
            return tokens;
        };
        match head {
            TokenTree::Token(token, _) => match token.kind {
                // `.ident` (field access) or `.0` (tuple index).
                // Postfix `.await` is *not* a field access — it's
                // `ExprKind::Await`, which the rule docs list as
                // non-trivial. Reject the `await` keyword explicitly so
                // `future.await` correctly falls out as non-trivial.
                // (`r#await` as a raw ident remains a literal field
                // access and stays accepted via the catch-all arm.)
                TokenKind::Dot => {
                    let Some((next, after)) = rest.split_first() else {
                        return tokens;
                    };
                    let TokenTree::Token(next_token, _) = next else {
                        return tokens;
                    };
                    match next_token.kind {
                        TokenKind::Ident(name, IdentIsRaw::No) if name == kw::Await => {
                            return tokens;
                        }
                        TokenKind::Ident(_, _) | TokenKind::Literal(_) => tokens = after,
                        _ => return tokens,
                    }
                }
                // `as path` — type annotation. Only path-shaped types
                // are recognised; references, slices, function pointers,
                // etc. fall back to non-trivial.
                TokenKind::Ident(name, IdentIsRaw::No) if name == kw::As => {
                    let Some(after) = take_trivial_type(rest) else {
                        return tokens;
                    };
                    tokens = after;
                }
                _ => return tokens,
            },
            // `[expr]` — index. Both base and index must be trivial;
            // the recursion happens here for the index.
            TokenTree::Delimited(_, _, Delimiter::Bracket, inner) => {
                if !is_trivial_expression_stream(inner) {
                    return tokens;
                }
                tokens = rest;
            }
            _ => return tokens,
        }
    }
}

fn take_trivial_type(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = head else {
        return None;
    };
    match token.kind {
        TokenKind::Ident(_, _) => Some(take_path_tail(rest)),
        TokenKind::PathSep => take_path_after_sep(rest),
        _ => None,
    }
}

fn is_trivial_expression_stream(stream: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = stream.iter().cloned().collect();
    is_trivial_expression(&trees)
}
