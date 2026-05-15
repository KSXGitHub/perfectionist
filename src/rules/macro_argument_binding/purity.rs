//! Argument splitting and the pure-expression predicate.
//!
//! [`split_top_level_arguments`] turns the macro invocation's
//! token stream into one segment per comma-separated argument.
//! [`looks_like_expression`] rules out non-expression positions the
//! macro author chose (`Type => [...]`, `name = value`, `name += value`,
//! `lhs -> rhs` arrow-paired matchers, `name in name`-style separators,
//! bare operators like `==`, and friends).
//! [`is_pure_expression`] decides whether the surviving expression
//! falls in the spec's pure shapes (literals, paths, references,
//! field accesses, indexing, dereferences, casts, parenthesised /
//! tuple groups, array literals and array-repeat forms over pure
//! parts, binary chains over pure operands, and `expr.method()`
//! postfixes for a curated / configured set of pure-getter
//! methods).
//!
//! The predicate is a hand-rolled token-stream walker — see the
//! rationale in `planned-rules/macro-argument-binding.md`'s
//! "Implementation notes" section. The walker is `take_*`-style per
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`.

use std::collections::BTreeSet;

use rustc_ast::token::{Delimiter, IdentIsRaw, Token, TokenKind};
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_span::kw;

/// Bundle of the two name-set tables the purity walker consults:
/// the pure-getter method names accepted as `.method()` postfixes,
/// and the compile-time-pure macro names accepted as `name!(...)`
/// atoms. Both are tail-segment-keyed (single-segment matching) and
/// owned by the rule's [`super::config::MacroArgumentBinding`] state;
/// passing them as one borrow keeps the recursive walker's signatures
/// short.
#[derive(Clone, Copy)]
pub(super) struct PurityContext<'a> {
    pub methods: &'a BTreeSet<String>,
    pub macros: &'a BTreeSet<String>,
}

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
///    shaped matchers); `->` (`link!("src" -> "dst")`-style arrow
///    matchers); the `in` keyword (`for_each!(x in iter, ...)`-style
///    matchers) — fails the check. `name = value` is technically a
///    valid Rust assignment expression of unit type, but in macro-
///    argument position the macro author overwhelmingly chose the `=`
///    as a structural marker; the let-bind rewrite the rule would
///    propose is meaningless for the macro's matcher arm. The same
///    reasoning extends to `->`, `in`, and the compound-assignment
///    family — in the shapes the rule observes, none of these tokens
///    form a single Rust expression standalone. `==`, by contrast,
///    is a real Rust binary operator (`debug_assert!(a == b)` is the
///    motivating pure shape) and is intentionally absent from this
///    list.
///
///    The trade-off is asymmetric. `->` and `in` *can* legitimately
///    appear inside a real Rust expression — `->` in a closure return
///    type (`|x: u32| -> u32 { x + 1 }`), `in` in a `for`-loop
///    expression (`for x in iter { ... }`). Both are impure
///    expressions the rule would otherwise flag; with the markers in
///    place the rule now silently *skips* them (false negative)
///    rather than emit a confusing `let`-bind hint inside a DSL
///    matcher (false positive). The latter has been reported in the
///    wild; the former has not. A future re-parse-based
///    implementation (see issue #64) will subsume the whole
///    heuristic and resolve the trade-off properly.
pub(super) fn looks_like_expression(argument: &[TokenTree]) -> bool {
    if let Some(TokenTree::Token(token, _)) = argument.first()
        && !token.can_begin_expr()
    {
        return false;
    }
    if argument.iter().any(|tree| match tree {
        TokenTree::Token(token, _) => is_dsl_marker(token),
        _ => false,
    }) {
        return false;
    }
    // A single brace-delimited argument is a block expression in
    // Rust, but in macro-argument position it's overwhelmingly the
    // outer carrier for a DSL body — `json!({"k": "v"})`,
    // `hashmap!({"k" => v})`, and similar. Descend one level and
    // look for DSL markers that wouldn't appear at the top level of
    // a real Rust block. If any are present, the argument is not
    // an expression the rule can rewrite, and `let`-binding it
    // wouldn't compile.
    if let [TokenTree::Delimited(_, _, Delimiter::Brace, inner)] = argument
        && brace_inner_looks_like_dsl(inner)
    {
        return false;
    }
    true
}

/// Heuristic: does a brace-delimited block's inner top level look
/// like a DSL body rather than a Rust statement list?
///
/// - `=>` at top level is always a DSL marker. The Rust block
///   grammar never produces a top-level `=>`: match arms live one
///   delimiter level deeper than the surrounding block.
/// - `:` at top level is a DSL key-position marker unless its
///   *statement* begins with `let`. The only Rust block
///   construct that emits a top-level `:` is the
///   `let pattern: type` annotation; struct literals (`Foo { x: 1 }`)
///   put the `:` one delimiter level deeper than the surrounding
///   block.
///
/// Statements are split by top-level `;`. At each statement's
/// start, leading outer / inner attributes (`#[cfg(...)]`,
/// `#![allow(...)]`) and doc comments are skipped before the
/// `let`-whitelist check, so `{ #[cfg(foo)] let x: T = e; x }`
/// (an attribute-annotated `let` binding inside a real block) is
/// still treated as a Rust block.
///
/// Known false-positive shapes — block-statement-starting tokens
/// other than `let` that legitimately introduce a top-level `:`
/// — are not currently whitelisted:
///
/// - `const NAME: type = expr;` and `static NAME: type = expr;`
///   item declarations inside the block.
/// - Labelled loops and blocks (`'a: loop { ... }`,
///   `'a: { ... }`) at the brace's immediate top level — the
///   lifetime token is followed by `:` in the same position as
///   a DSL key.
///
/// All three constructs are vanishingly rare in macro-argument
/// position; the trade-off keeps the heuristic simple at the
/// price of a documented false skip on these shapes.
fn brace_inner_looks_like_dsl(stream: &TokenStream) -> bool {
    let trees: Vec<&TokenTree> = stream.iter().collect();
    let mut whitelist_let = false;
    let mut cursor = 0;
    while cursor < trees.len() {
        // Statement boundary: re-evaluate the `let`-whitelist on
        // the new statement's first non-attribute token.
        let after_attrs = skip_leading_attributes(&trees, cursor);
        if let Some(TokenTree::Token(token, _)) = trees.get(after_attrs)
            && token.is_keyword(kw::Let)
        {
            whitelist_let = true;
        }
        cursor = after_attrs;
        // Walk the statement body until `;` or end of stream.
        while cursor < trees.len() {
            if let TokenTree::Token(token, _) = trees[cursor] {
                match token.kind {
                    TokenKind::Semi => {
                        whitelist_let = false;
                        cursor += 1;
                        break;
                    }
                    TokenKind::FatArrow => return true,
                    TokenKind::Colon if !whitelist_let => return true,
                    _ => {}
                }
            }
            cursor += 1;
        }
    }
    false
}

/// Advance past any leading outer (`#[...]`) or inner (`#![...]`)
/// attributes and doc comments at position `start`, returning the
/// index of the first non-attribute token tree. The check at
/// statement start uses this so leading attributes don't disable
/// the `let`-whitelist for the `:` in a `let pattern: type`
/// binding.
fn skip_leading_attributes(trees: &[&TokenTree], mut start: usize) -> usize {
    loop {
        let Some(tree) = trees.get(start) else {
            return start;
        };
        let TokenTree::Token(token, _) = tree else {
            return start;
        };
        match token.kind {
            TokenKind::DocComment(..) => start += 1,
            TokenKind::Pound => {
                let mut after_pound = start + 1;
                if matches!(trees.get(after_pound), Some(TokenTree::Token(t, _)) if t.kind == TokenKind::Bang)
                {
                    after_pound += 1;
                }
                if matches!(
                    trees.get(after_pound),
                    Some(TokenTree::Delimited(_, _, Delimiter::Bracket, _)),
                ) {
                    start = after_pound + 1;
                } else {
                    return start;
                }
            }
            _ => return start,
        }
    }
}

fn is_dsl_marker(token: &Token) -> bool {
    if token.is_keyword(kw::In) {
        return true;
    }
    matches!(
        token.kind,
        TokenKind::FatArrow
            | TokenKind::RArrow
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

/// Returns `true` if the entire token slice forms a "pure"
/// expression per the rule's grammar. Purity is purely syntactic:
/// the shapes the rule docs enumerate (literal, path, reference,
/// field, index, deref, cast), plus parenthesised / tuple groups
/// whose elements are all pure, plus binary chains whose every
/// operand is pure, plus `.method()` postfixes when the method
/// name is in `pure_methods` (curated pure-getter set, extensible
/// via `dylint.toml`). The classification is recursive on operands.
/// Anything outside that grammar is impure — including most
/// `const fn` calls, generic method calls, and other "morally pure"
/// expressions the walker cannot prove side-effect-free.
pub(super) fn is_pure_expression(tokens: &[TokenTree], ctx: PurityContext<'_>) -> bool {
    take_pure_expression(tokens, ctx).is_some_and(<[_]>::is_empty)
}

fn take_pure_expression<'a>(
    tokens: &'a [TokenTree],
    ctx: PurityContext<'_>,
) -> Option<&'a [TokenTree]> {
    let after_atom = take_pure_atom(tokens, ctx)?;
    let after_suffix = take_pure_suffixes(after_atom, ctx);
    Some(take_pure_binary_tail(after_suffix, ctx))
}

fn take_pure_atom<'a>(tokens: &'a [TokenTree], ctx: PurityContext<'_>) -> Option<&'a [TokenTree]> {
    let (head, rest) = tokens.split_first()?;
    match head {
        // `()` (unit literal), `(expr)` (parenthesised pure
        // expression), `(a, b)` / `(a,)` (tuple of pure elements).
        // Each element is recursively pure; empty parens are the
        // canonical pure value.
        //
        // The match is restricted to `Delimiter::Parenthesis` —
        // `Delimiter::Invisible` (capture-wrapping delimiters
        // introduced by macro expansion) falls through to the
        // bottom `_ => None` arm. That's correct here: the rule
        // runs pre-expansion and never sees invisible delimiters
        // in practice.
        TokenTree::Delimited(_, _, Delimiter::Parenthesis, inner) => {
            if is_pure_paren_inner(inner, ctx) {
                Some(rest)
            } else {
                None
            }
        }
        // `[]` (empty array), `[a, b, ...]` (array literal with
        // optional trailing comma), `[expr; count]` (array repeat).
        // Each element is recursively pure; the repeat form
        // requires both halves to be pure. The indexing suffix
        // `base[index]` is handled separately by
        // `take_pure_suffixes` — it never reaches this arm because
        // an indexed expression starts with the base path, not the
        // bracket.
        TokenTree::Delimited(_, _, Delimiter::Bracket, inner) => {
            if is_pure_array_inner(inner, ctx) {
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
            TokenKind::And => take_reference_tail(rest, ctx),
            // `&&` expr or `&& mut` expr (double reference).
            TokenKind::AndAnd => take_reference_tail(rest, ctx),
            // `*expr` (deref).
            TokenKind::Star => take_pure_expression(rest, ctx),
            // Path: ident (`::` ident)*. If the path is followed by
            // `!` and the path's final segment names a curated
            // pure macro (`concat!`, `env!`, `include_str!`, ...),
            // the whole `name!(...)` / `name![...]` is a pure
            // atom — the expansion is a compile-time constant and
            // the macro itself does not evaluate any runtime user
            // expression (`stringify!` takes a token sequence and
            // `cfg!` takes a cfg predicate, but neither evaluates
            // them as Rust code). The body contents are therefore
            // unchecked: there is no runtime expression for the
            // surrounding macro to drop or duplicate, regardless of
            // what the inner macro's input shape is.
            TokenKind::Ident(name, _) => {
                Some(take_path_and_optional_macro_call(name, rest, ctx.macros))
            }
            // Leading `::` — must be followed by an ident.
            TokenKind::PathSep => take_atom_path_after_sep(rest, ctx.macros),
            _ => None,
        },
        _ => None,
    }
}

/// Accept `()` (empty, the unit literal), `(expr)` (parenthesised),
/// `(a, b, ...)` (tuple, optional trailing comma) when every element is
/// itself pure. Empty elements in the middle (`(a,,b)`) are not
/// Rust syntax and are rejected.
fn is_pure_paren_inner(stream: &TokenStream, ctx: PurityContext<'_>) -> bool {
    let Some(arguments) = split_top_level_arguments(stream) else {
        return false;
    };
    arguments
        .iter()
        .all(|argument| !argument.is_empty() && is_pure_expression(argument, ctx))
}

/// Accept `[]` (empty array literal), `[a, b, ...]` (array literal,
/// optional trailing comma), and `[expr; count]` (array repeat) when
/// every contained expression is itself pure. The repeat form is
/// recognised by the top-level `;`; mixing `;` and `,` at the top
/// level is malformed and rejected.
fn is_pure_array_inner(stream: &TokenStream, ctx: PurityContext<'_>) -> bool {
    if let Some(arguments) = split_top_level_arguments(stream) {
        return arguments
            .iter()
            .all(|argument| !argument.is_empty() && is_pure_expression(argument, ctx));
    }
    let Some((expr, count)) = split_array_repeat(stream) else {
        return false;
    };
    !expr.is_empty()
        && is_pure_expression(&expr, ctx)
        && !count.is_empty()
        && is_pure_expression(&count, ctx)
}

/// Split a bracket-delimited stream at the first top-level `;`,
/// the array-repeat separator. Returns `None` if the stream has
/// no top-level `;`, more than one top-level `;`, or any top-level
/// `,` (the repeat form is `[expr; count]` exactly — a comma at
/// the top level signals a malformed mixture with array-literal
/// syntax).
fn split_array_repeat(stream: &TokenStream) -> Option<(Vec<TokenTree>, Vec<TokenTree>)> {
    let mut before: Vec<TokenTree> = Vec::new();
    let mut after: Option<Vec<TokenTree>> = None;
    for tree in stream.iter() {
        if let TokenTree::Token(token, _) = tree {
            match token.kind {
                TokenKind::Semi => {
                    if after.is_some() {
                        return None;
                    }
                    after = Some(Vec::new());
                    continue;
                }
                TokenKind::Comma => return None,
                _ => {}
            }
        }
        match after.as_mut() {
            Some(buf) => buf.push(tree.clone()),
            None => before.push(tree.clone()),
        }
    }
    after.map(|count| (before, count))
}

fn take_reference_tail<'a>(
    tokens: &'a [TokenTree],
    ctx: PurityContext<'_>,
) -> Option<&'a [TokenTree]> {
    let after_mut = match tokens.split_first() {
        Some((TokenTree::Token(token, _), rest)) if token.is_keyword(kw::Mut) => rest,
        _ => tokens,
    };
    take_pure_expression(after_mut, ctx)
}

/// Walk a path tail beginning after a leading ident, tracking the
/// path's final segment so the caller can match it against the
/// pure-macro list when a `!` follows. The first segment's name
/// is passed in; the walker reads `::ident` runs and returns the
/// last segment seen along with the slice after the path.
fn walk_path_tail(
    first_name: rustc_span::Symbol,
    mut tokens: &[TokenTree],
) -> (rustc_span::Symbol, &[TokenTree]) {
    let mut last = first_name;
    while let Some((TokenTree::Token(sep, _), after_sep)) = tokens.split_first() {
        if sep.kind != TokenKind::PathSep {
            break;
        }
        let Some((TokenTree::Token(ident, _), after_ident)) = after_sep.split_first() else {
            break;
        };
        let TokenKind::Ident(name, _) = ident.kind else {
            break;
        };
        last = name;
        tokens = after_ident;
    }
    (last, tokens)
}

fn take_path_tail(mut tokens: &[TokenTree]) -> &[TokenTree] {
    // Type-position paths and `as`-cast paths don't need to know the
    // tail segment's name (no trailing `!` is recognised there), so
    // this variant skips the tracking that `walk_path_tail` does.
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

/// After consuming the leading ident of an atom path, walk the rest
/// of the path and optionally consume a trailing pure macro call.
/// `first_name` is the leading ident's symbol; `tokens` is the slice
/// following it.
fn take_path_and_optional_macro_call<'a>(
    first_name: rustc_span::Symbol,
    tokens: &'a [TokenTree],
    pure_macros: &BTreeSet<String>,
) -> &'a [TokenTree] {
    let (tail_name, after_path) = walk_path_tail(first_name, tokens);
    take_pure_macro_call(after_path, tail_name, pure_macros).unwrap_or(after_path)
}

/// If `tokens` starts with `! ( ... )` or `! [ ... ]` AND
/// `macro_name`'s final segment is in `pure_macros`, consume the
/// `!` and the delimited body and return the slice after it.
/// Returns `None` otherwise; the caller falls back to leaving the
/// tokens unconsumed (so an impure macro call correctly drops
/// the whole expression into the impure bucket).
fn take_pure_macro_call<'a>(
    tokens: &'a [TokenTree],
    macro_name: rustc_span::Symbol,
    pure_macros: &BTreeSet<String>,
) -> Option<&'a [TokenTree]> {
    let (bang, after_bang) = tokens.split_first()?;
    let TokenTree::Token(bang_token, _) = bang else {
        return None;
    };
    if bang_token.kind != TokenKind::Bang {
        return None;
    }
    if !pure_macros.contains(macro_name.as_str()) {
        return None;
    }
    let (delim, after_delim) = after_bang.split_first()?;
    let TokenTree::Delimited(_, _, delim_kind, _) = delim else {
        return None;
    };
    // Curly-delimited inner macros are out of scope for the same
    // reason curly-delimited outer macros are: `name! { ... }` is
    // conventionally a DSL body, not a value-producing call.
    if !matches!(delim_kind, Delimiter::Parenthesis | Delimiter::Bracket) {
        return None;
    }
    Some(after_delim)
}

fn take_atom_path_after_sep<'a>(
    tokens: &'a [TokenTree],
    pure_macros: &BTreeSet<String>,
) -> Option<&'a [TokenTree]> {
    let (ident, rest) = tokens.split_first()?;
    let TokenTree::Token(token, _) = ident else {
        return None;
    };
    let TokenKind::Ident(name, _) = token.kind else {
        return None;
    };
    Some(take_path_and_optional_macro_call(name, rest, pure_macros))
}

/// Type-position path tail: `take_atom_path_after_sep`'s sibling
/// for `take_pure_type`. Types don't carry trailing `!` macro
/// calls so this variant only walks the `::ident` chain.
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

fn take_pure_suffixes<'a>(mut tokens: &'a [TokenTree], ctx: PurityContext<'_>) -> &'a [TokenTree] {
    loop {
        let Some((head, rest)) = tokens.split_first() else {
            return tokens;
        };
        match head {
            TokenTree::Token(token, _) => match token.kind {
                // `.ident` (field access), `.0` (tuple index), or
                // `.method()` (zero-arg pure-getter call) when the
                // method name is in the configured pure-methods
                // set. Postfix `.await` is *not* a field access —
                // it's `ExprKind::Await`, which the rule docs list as
                // impure. Reject the `await` keyword explicitly
                // so `future.await` correctly falls out as impure.
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
                        TokenKind::Ident(name, IdentIsRaw::No) => {
                            // `.name()` (empty parens) is a pure
                            // postfix when the method is conventionally
                            // pure (`vec.len()`, `s.is_empty()`,
                            // `opt.as_ref()`). Otherwise treat `.name`
                            // as a plain field access and let the
                            // suffix loop decide what to do with the
                            // tokens that follow.
                            if is_pure_method_call(after, name.as_str(), ctx.methods) {
                                tokens = &after[1..];
                            } else {
                                tokens = after;
                            }
                        }
                        // Raw idents (`r#len`, `r#as_ref`) and tuple
                        // indices fall through to plain field-access
                        // handling. The pure-getter set keys off
                        // non-raw idents because Rust doesn't reserve
                        // any of the curated names, so a raw-escaped
                        // form is unusual enough not to be worth the
                        // extra branch.
                        TokenKind::Ident(_, _) | TokenKind::Literal(_) => tokens = after,
                        _ => return tokens,
                    }
                }
                // `as path` — type annotation. Only path-shaped types
                // are recognised; references, slices, function pointers,
                // etc. fall back to impure.
                TokenKind::Ident(name, IdentIsRaw::No) if name == kw::As => {
                    let Some(after) = take_pure_type(rest) else {
                        return tokens;
                    };
                    tokens = after;
                }
                _ => return tokens,
            },
            // `[expr]` — index. Both base and index must be pure;
            // the recursion happens here for the index.
            TokenTree::Delimited(_, _, Delimiter::Bracket, inner) => {
                if !is_pure_expression_stream(inner, ctx) {
                    return tokens;
                }
                tokens = rest;
            }
            _ => return tokens,
        }
    }
}

/// `true` iff `tokens` starts with `()` (empty parentheses) AND the
/// preceding method name is in `pure_methods`. The caller has
/// already consumed the `.` and the method ident; it passes the
/// remaining tokens (starting with the `(...)` delimiter) here.
fn is_pure_method_call(
    tokens: &[TokenTree],
    method_name: &str,
    pure_methods: &BTreeSet<String>,
) -> bool {
    let Some(TokenTree::Delimited(_, _, Delimiter::Parenthesis, inner)) = tokens.first() else {
        return false;
    };
    inner.is_empty() && pure_methods.contains(method_name)
}

/// Consume a tail of `OP pure` pairs where `OP` is a side-effect-
/// free binary operator (arithmetic, bitwise, comparison, logical).
/// The spec's "impure" boundary explicitly couples binary
/// expression purity to operand purity: `a <= b` and
/// `count + offset` are side-effect-free over pure operands and
/// should themselves be pure. Without this tail, simple comparisons
/// in `debug_assert!(a <= b)` would be flagged and the suggested `let`
/// binding would force the comparison to evaluate in release builds —
/// the opposite of the user's intent.
///
/// The walker does not honour Rust's binary-operator precedence
/// (`a + b * c` is consumed left-to-right rather than as `a + (b * c)`),
/// but that does not affect the purity verdict: every prefix /
/// suffix in the chain has pure operands.
fn take_pure_binary_tail<'a>(
    mut tokens: &'a [TokenTree],
    ctx: PurityContext<'_>,
) -> &'a [TokenTree] {
    while let Some(after_op) = take_pure_binary_operator(tokens) {
        let Some(after_atom) = take_pure_atom(after_op, ctx) else {
            // The operator looked like a binop but no pure atom
            // followed; leave the operator unconsumed so the caller
            // sees the whole rest as impure.
            return tokens;
        };
        tokens = take_pure_suffixes(after_atom, ctx);
    }
    tokens
}

fn take_pure_binary_operator(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
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

fn take_pure_type(tokens: &[TokenTree]) -> Option<&[TokenTree]> {
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

fn is_pure_expression_stream(stream: &TokenStream, ctx: PurityContext<'_>) -> bool {
    let trees: Vec<TokenTree> = stream.iter().cloned().collect();
    is_pure_expression(&trees, ctx)
}
