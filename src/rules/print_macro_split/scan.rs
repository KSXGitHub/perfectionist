//! Locating the format template in a macro invocation's token stream
//! and turning a long single-line call into the wrapped,
//! backslash-newline-continued form.
//!
//! The fold is escape-aware: it walks the literal body with the shared
//! [`crate::literal_scan::take_string_escape`] combinator so a `\\n`
//! (escaped backslash, then the letter `n`) is never mistaken for the
//! `\n` newline escape it folds. The rewrite is byte-for-byte
//! output-preserving — every folded `\n` keeps its newline and the
//! trailing `\<newline><indent>` continuation strips exactly the
//! source newline and indentation it introduced.

use rustc_ast::MacCall;
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::TokenTree;
use rustc_span::Span;
use rustc_span::source_map::SourceMap;

use crate::common::display_width;
use crate::literal_scan::take_string_escape;

/// One indentation step. The wrapped form indents the argument list
/// one level past the line the invocation starts on; four spaces
/// matches rustfmt's default.
const INDENT_STEP: &str = "    ";

/// Build the `(call_span, replacement)` for the wrapped,
/// continuation-folded form, or `None` if the invocation should be
/// left alone — its start line is within `max_line_width`, the
/// template has no foldable interior `\n`, or a span could not be
/// resolved to source text.
pub(super) fn build_fold_suggestion(
    source_map: &SourceMap,
    mac_call: &MacCall,
    template_span: Span,
    max_line_width: usize,
) -> Option<(Span, String)> {
    let open_span = mac_call.args.dspan.open;
    let close_span = mac_call.args.dspan.close;
    let call_span = mac_call.path.span.to(mac_call.args.dspan.entire());

    // Indentation and display width of the line the call starts on.
    let location = source_map.lookup_char_pos(call_span.lo());
    let line_index = location.line.checked_sub(1)?;
    let line_text = location.file.get_line(line_index)?;
    if display_width(&line_text) <= max_line_width {
        return None;
    }
    let indent_len = line_text.len() - line_text.trim_start().len();
    let base_indent = line_text[..indent_len].to_owned();
    let inner_indent = format!("{base_indent}{INDENT_STEP}");

    // Fold the template body. Bails when nothing is foldable so the
    // diagnostic never fires without an accompanying rewrite.
    let template_snippet = source_map.span_to_snippet(template_span).ok()?;
    let body = template_snippet
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))?;
    let folded_body = fold_template_body(body, &inner_indent)?;
    let folded_literal = format!(r#""{folded_body}""#);

    // Splice the folded literal back into the delimited argument list,
    // keeping every other argument verbatim.
    let header = source_map
        .span_to_snippet(call_span.with_hi(open_span.hi()))
        .ok()?;
    let footer = source_map.span_to_snippet(close_span).ok()?;
    let inner = source_map
        .span_to_snippet(open_span.between(close_span))
        .ok()?;

    // The wrapped form is multi-line, so rustfmt's vertical policy wants
    // a trailing comma after the last argument. Insert it right after
    // the last token (unless one is already there) rather than at the
    // textual end of `inner`: a trailing line comment is not a token, so
    // appending at the end would bury the comma inside the comment.
    let last_token = mac_call.args.tokens.iter().last()?;
    let already_has_trailing_comma =
        matches!(last_token, TokenTree::Token(token, _) if token.kind == TokenKind::Comma);
    let inner = if already_has_trailing_comma {
        inner
    } else {
        let comma_at = (last_token.span().hi().0.checked_sub(open_span.hi().0)?) as usize;
        let (head, tail) = (inner.get(..comma_at)?, inner.get(comma_at..)?);
        format!("{head},{tail}")
    };

    // The comma is anchored at the last top-level token tree, while the
    // template is itself a top-level token tree (`find_template_literal`
    // only ever selects a lone `TokenTree::Token`, never a nested group)
    // at or before it in source order. So `comma_at >= template_hi`: the
    // insertion above leaves the `[..template_hi]` prefix — and therefore
    // both template offsets computed below — untouched.
    let template_lo = (template_span.lo().0.checked_sub(open_span.hi().0)?) as usize;
    let template_hi = (template_span.hi().0.checked_sub(open_span.hi().0)?) as usize;
    let before_template = inner.get(..template_lo)?;
    let after_template = inner.get(template_hi..)?;
    let mut new_inner = format!("{before_template}{folded_literal}{after_template}");
    new_inner.truncate(new_inner.trim_end().len());

    let replacement = format!("{header}\n{inner_indent}{new_inner}\n{base_indent}{footer}");
    Some((call_span, replacement))
}

/// Fold every *interior* `\n` newline escape in a cooked literal body
/// into the `\n\<newline><indent>` continuation form, returning the
/// rewritten body — or `None` if nothing was folded.
///
/// A `\n` is folded only when the source text immediately after it is
/// neither end-of-body (a trailing `\n` needs no fold) nor a literal
/// whitespace character. The whitespace guard is load-bearing for
/// byte-equivalence: the `\<newline>` continuation strips the newline
/// *and all leading whitespace* of the next source line, so folding
/// `"a\n  b"` would silently swallow the two spaces of the second
/// fragment. An escape such as `\t` is safe — its leading `\` is not
/// whitespace, so the continuation stops there and the escape
/// survives.
fn fold_template_body(body: &str, continuation_indent: &str) -> Option<String> {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    let mut folded = false;
    while !rest.is_empty() {
        if let Some((escape, remainder)) = take_string_escape(rest) {
            let following_is_whitespace = remainder.chars().next().is_some_and(char::is_whitespace);
            if escape == r"\n" && !remainder.is_empty() && !following_is_whitespace {
                out.push_str("\\n\\\n");
                out.push_str(continuation_indent);
                folded = true;
            } else {
                out.push_str(escape);
            }
            rest = remainder;
            continue;
        }
        let next = rest.chars().next()?;
        let char_len = next.len_utf8();
        out.push_str(&rest[..char_len]);
        rest = &rest[char_len..];
    }
    folded.then_some(out)
}
