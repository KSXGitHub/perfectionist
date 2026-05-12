//! Argument splitting and the trivial-expression predicate.
//!
//! [`split_top_level_arguments`] turns the macro invocation's
//! token stream into one segment per comma-separated argument.
//! [`looks_like_expression`] rules out non-expression positions the
//! macro author chose (`Type => [...]`, `name = value`, `name += value`,
//! bare operators like `==`, and friends).
//! [`is_trivial_expression`] decides whether the surviving expression
//! falls in the spec's seven trivial shapes (plus parenthesised /
//! tuple groups and binary chains over trivial operands).
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
/// inputs:
///
/// 1. The first token must be one that can begin a Rust expression.
///    A bare operator token like `==` in
///    `debug_assert_op_expr!(a, ==, b)` is not an expression at all;
///    suggesting a `let` binding for it is nonsensical, so the rule
///    skips the argument.
/// 2. A top-level token that signals a DSL pattern — `=>` (match-arm
///    separator inside `matches!`, `impl_lint_pass!`-style
///    `Type => [LINT_NAMES]` DSLs); `=`, `+=`, `-=`, ... (assignment-
///    shaped matchers like `make_const!(NAME = '█')` or
///    `bump!(items += 1)`); a top-level `:` (`name: type` ascription-
///    shaped matchers) — fails the check. `name = value` is technically
///    a valid Rust assignment expression of unit type, but in macro-
///    argument position the macro author overwhelmingly chose the `=`
///    as a structural marker; the let-bind rewrite the rule would
///    propose is meaningless for the macro's matcher arm. A future
///    re-parse-based implementation will subsume this check.
pub(super) fn looks_like_expression(argument: &[TokenTree]) -> bool {
    if let Some(TokenTree::Token(token, _)) = argument.first()
        && !token.can_begin_expr()
    {
        return false;
    }
    !argument.iter().any(|tree| match tree {
        TokenTree::Token(token, _) => is_dsl_marker(token.kind),
        _ => false,
    })
}

fn is_dsl_marker(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::FatArrow
            | TokenKind::Colon
            | TokenKind::Eq
            | TokenKind::PlusEq
            | TokenKind::MinusEq
            | TokenKind::StarEq
            | TokenKind::SlashEq
            | TokenKind::PercentEq
            | TokenKind::AndEq
            | TokenKind::OrEq
            | TokenKind::CaretEq
            | TokenKind::ShlEq
            | TokenKind::ShrEq,
    )
}

/// Returns `true` if the entire token slice forms a "trivial"
/// expression per the rule's grammar. Triviality is purely syntactic:
/// the seven shapes the rule docs enumerate (literal, path, reference,
/// field, index, deref, cast), plus parenthesised / tuple groups whose
/// elements are all trivial, plus binary chains whose every operand is
/// trivial. The classification is recursive on operands. Anything
/// outside that grammar is non-trivial — including `const fn` calls
/// and other "morally pure" expressions.
pub(super) fn is_trivial_expression(tokens: &[TokenTree]) -> bool {
    take_trivial_expression(tokens).is_some_and(<[_]>::is_empty)
}

fn take_trivial_expression(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let after_atom = take_trivial_atom(tokens)?;
    let after_suffix = take_trivial_suffixes(after_atom);
    Some(take_trivial_binary_tail(after_suffix))
}

fn take_trivial_atom(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    match head {
        // `()` (unit literal), `(expr)` (parenthesised trivial
        // expression), `(a, b)` / `(a,)` (tuple of trivial elements).
        // Each element is recursively trivial; empty parens are the
        // canonical trivial value.
        TokenTree::Delimited(_, _, Delimiter::Parenthesis, inner) => {
            if is_trivial_paren_inner(inner) {
                Some(rest)
            } else {
                None
            }
        }
        TokenTree::Token(token, _) => match token.kind {
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
        },
        _ => None,
    }
}

/// Accept `()` (empty, the unit literal), `(expr)` (parenthesised),
/// `(a, b, ...)` (tuple, optional trailing comma) when every element is
/// itself trivial. Empty elements in the middle (`(a,,b)`) are not
/// Rust syntax and are rejected.
fn is_trivial_paren_inner(stream: &TokenStream) -> bool {
    let Some(arguments) = split_top_level_arguments(stream) else {
        return false;
    };
    arguments
        .iter()
        .all(|argument| !argument.is_empty() && is_trivial_expression(argument))
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

/// Consume a tail of `OP trivial` pairs where `OP` is a side-effect-
/// free binary operator (arithmetic, bitwise, comparison, logical).
/// The spec's "non-trivial" boundary explicitly couples binary
/// expression triviality to operand triviality: `a <= b` and
/// `count + offset` are side-effect-free over trivial operands and
/// should themselves be trivial. Without this tail, simple comparisons
/// in `debug_assert!(a <= b)` would be flagged and the suggested `let`
/// binding would force the comparison to evaluate in release builds —
/// the opposite of the user's intent.
///
/// The walker does not honour Rust's binary-operator precedence
/// (`a + b * c` is consumed left-to-right rather than as `a + (b * c)`),
/// but that does not affect the triviality verdict: every prefix /
/// suffix in the chain has trivial operands.
fn take_trivial_binary_tail(mut tokens: &[TokenTree]) -> &[TokenTree] {
    while let Some(after_op) = take_trivial_binary_operator(tokens) {
        let Some(after_atom) = take_trivial_atom(after_op) else {
            // The operator looked like a binop but no trivial atom
            // followed; leave the operator unconsumed so the caller
            // sees the whole rest as non-trivial.
            return tokens;
        };
        tokens = take_trivial_suffixes(after_atom);
    }
    tokens
}

fn take_trivial_binary_operator(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = head else {
        return None;
    };
    matches!(
        token.kind,
        TokenKind::EqEq
            | TokenKind::Ne
            | TokenKind::Lt
            | TokenKind::Gt
            | TokenKind::Le
            | TokenKind::Ge
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Caret
            | TokenKind::And
            | TokenKind::Or
            | TokenKind::Shl
            | TokenKind::Shr,
    )
    .then_some(rest)
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
