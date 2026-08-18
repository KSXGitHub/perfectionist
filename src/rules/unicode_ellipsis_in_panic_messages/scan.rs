//! The two source-text scans the rule performs.
//!
//! [`scan_macro_call_source`] walks the rendered text of a panic /
//! assertion macro invocation with `rustc_lexer::tokenize`, tracks
//! delimiter nesting so only depth-1 literals reach the per-character
//! scan, and skips over the leading non-message arguments of
//! `assert!`-family macros.
//!
//! [`scan_literal`] is the simpler entry point used directly by
//! `expect` / `expect_err`: take a string-literal snippet, locate its
//! body, and forward to [`crate::literal_scan::emit_flagged_chars`].

use super::UNICODE_ELLIPSIS_IN_PANIC_MESSAGES;
use crate::literal_scan::{emit_flagged_chars, string_literal_quote_lengths};
use rustc_lexer::{FrontmatterAllowed, LiteralKind, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Pos, Span, Symbol};

pub(super) fn scan_macro_call_source(
    lint_context: &LateContext<'_>,
    flagged_chars: &[char],
    call_span: Span,
    macro_name: Symbol,
) {
    let Ok(snippet) = lint_context.sess().source_map().span_to_snippet(call_span) else {
        return;
    };
    let context = format!("`{macro_name}!` message");
    // Track delimiter nesting so we only scan literals at the
    // macro's own argument level. The snippet starts with
    // `macro_name!(`/`[`/`{`, which opens depth 1; literals
    // belonging to the panic message live at exactly depth 1.
    // Anything deeper is an argument of a nested call (e.g.,
    // `format!("...")` or `include_str!("path")`) whose literal
    // is not the panic message.
    //
    // Within depth 1 we also count commas to skip the non-message
    // arguments of `assert!`-family macros: `assert!(cond, msg)`
    // skips one comma-separated argument before the message,
    // `assert_eq!(a, b, msg)` and `assert_ne!`/`debug_assert_eq!`/
    // `debug_assert_ne!` skip two. Scanning value-position
    // literals would otherwise rewrite comparison operands.
    let skip_arguments = arguments_before_message(macro_name);
    let mut byte_offset: u32 = 0;
    let mut depth: u32 = 0;
    let mut top_level_comma_count: u32 = 0;
    for token in tokenize(&snippet, FrontmatterAllowed::No) {
        let token_length = token.len;
        match token.kind {
            TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace => {
                depth = depth.saturating_add(1);
            }
            TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace => {
                depth = depth.saturating_sub(1);
            }
            TokenKind::Comma if depth == 1 => {
                top_level_comma_count = top_level_comma_count.saturating_add(1);
            }
            TokenKind::Literal { kind, .. }
                if depth == 1
                    && top_level_comma_count >= skip_arguments
                    && is_display_string_literal(kind) =>
            {
                let token_start = byte_offset as usize;
                let token_end = token_start + token_length as usize;
                let literal_snippet = &snippet[token_start..token_end];
                let token_lo = call_span.lo() + BytePos::from_u32(byte_offset);
                let token_hi = token_lo + BytePos::from_u32(token_length);
                let token_span =
                    Span::new(token_lo, token_hi, call_span.ctxt(), call_span.parent());
                scan_literal(
                    lint_context,
                    flagged_chars,
                    token_span,
                    literal_snippet,
                    &context,
                );
            }
            _ => {}
        }
        byte_offset = byte_offset
            .checked_add(token_length)
            .expect("snippet offset overflowed u32");
    }
}

pub(super) fn scan_literal(
    lint_context: &LateContext<'_>,
    flagged_chars: &[char],
    literal_span: Span,
    literal_snippet: &str,
    context: &str,
) {
    let Some((prefix_length, suffix_length)) = string_literal_quote_lengths(literal_snippet) else {
        return;
    };
    let body = &literal_snippet[prefix_length..literal_snippet.len() - suffix_length];
    emit_flagged_chars(
        lint_context,
        UNICODE_ELLIPSIS_IN_PANIC_MESSAGES,
        body,
        flagged_chars,
        context,
        |byte_offset, character_length| {
            let span_start =
                literal_span.lo() + BytePos::from_u32((prefix_length + byte_offset) as u32);
            let span_end = span_start + BytePos::from_u32(character_length);
            Span::new(
                span_start,
                span_end,
                literal_span.ctxt(),
                literal_span.parent(),
            )
        },
    );
}

fn is_display_string_literal(kind: LiteralKind) -> bool {
    matches!(kind, LiteralKind::Str { .. } | LiteralKind::RawStr { .. })
}

/// How many comma-separated top-level arguments precede the message
/// argument for a given macro. `0` for macros whose first argument is
/// itself the message (`panic!`, `todo!`, ...); `1` for `assert!` /
/// `debug_assert!` (the condition comes first); `2` for `assert_eq!`
/// / `assert_ne!` / `debug_assert_eq!` / `debug_assert_ne!` (the two
/// values come first).
///
/// Unknown macros — including any added through the configuration's
/// `extra_macros` knob — default to `0`. A project that adds an
/// assertion-shaped custom macro through configuration accepts the
/// false positive in that case; correctly handling it would require
/// a per-macro skip-count configuration knob, which this rule
/// deliberately does not offer.
fn arguments_before_message(macro_name: Symbol) -> u32 {
    match macro_name.as_str() {
        "assert" | "debug_assert" => 1,
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne" => 2,
        _ => 0,
    }
}
