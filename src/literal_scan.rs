//! Helpers shared by rules that scan string-literal / comment text.
//!
//! [`emit_flagged_chars`] is used by the Unicode-ellipsis rules that
//! scan a contiguous stretch of text (`unicode_ellipsis_in_comments`
//! and `unicode_ellipsis_in_panic_messages`): walk it, emit a
//! diagnostic for each flagged character, and offer the same `...`
//! autofix. The per-character logic is identical; the only per-rule
//! pieces are the lint name, a context label, and how to turn a byte
//! offset within the text into a [`Span`].
//!
//! [`emit_flagged_char`] is the single-character core, factored out so
//! a rule that does its own scanning — `unicode_ellipsis_in_docs`,
//! which must consult a markdown code-region mask and a fallible
//! span map before emitting — shares the exact message, suggestion,
//! and applicability without duplicating them.
//!
//! [`string_literal_quote_lengths`] is the companion parser for any
//! rule that needs to scan a string-literal body without its opening
//! and closing delimiters. Currently used only by
//! `unicode_ellipsis_in_panic_messages`'s literal scanner, but it
//! sits here rather than inside that rule because the shape it
//! recognises (plain and raw display strings) is a generic property
//! of Rust string literals, not specific to ellipsis detection.

use clippy_utils::diagnostics::span_lint_and_sugg;
use rustc_errors::Applicability;
use rustc_lint::{Lint, LintContext};
use rustc_span::Span;

/// For each character in `text` that appears in `flagged`, emit a
/// diagnostic against `lint` with the suggested `...` replacement.
///
/// `context_label` is the trailing phrase in the message, e.g.
/// `"comment"` or `` "`panic!` message" ``. `span_for` maps a
/// `(byte_offset_within_text, character_utf8_length)` pair into the
/// [`Span`] of the offending character in source — different callers
/// resolve this differently (a source-file-relative position for the
/// comment scanner, a `BytePos`-arithmetic offset from a literal span
/// for the panic-message scanner).
///
/// Applicability is [`MachineApplicable`] for U+2026 (the rule's
/// primary target, which always maps cleanly to `...`) and
/// [`MaybeIncorrect`] for any user-configured `extra_flagged_chars`
/// entries (whose visual equivalence to `...` is up to the project to
/// assert).
///
/// [`MachineApplicable`]: Applicability::MachineApplicable
/// [`MaybeIncorrect`]: Applicability::MaybeIncorrect
pub(crate) fn emit_flagged_chars<Cx>(
    lint_context: &Cx,
    lint: &'static Lint,
    text: &str,
    flagged: &[char],
    context_label: &str,
    mut span_for: impl FnMut(usize, u32) -> Span,
) where
    Cx: LintContext,
{
    for (byte_offset, character) in text.char_indices() {
        if !flagged.contains(&character) {
            continue;
        }
        let character_length = character.len_utf8() as u32;
        let span = span_for(byte_offset, character_length);
        emit_flagged_char(lint_context, lint, character, span, context_label);
    }
}

/// Emit a single flagged-character diagnostic at `span`, suggesting
/// the ASCII `...` replacement. Factored out of [`emit_flagged_chars`]
/// so rules that run their own scan loop (the doc-comment scanner,
/// which filters against a code-region mask and a fallible span map)
/// reuse the same message text and applicability.
///
/// Applicability is [`MachineApplicable`] for U+2026 (the rules'
/// primary target, which always maps cleanly to `...`) and
/// [`MaybeIncorrect`] for any user-configured `extra_flagged_chars`
/// entry (whose visual equivalence to `...` is up to the project to
/// assert).
///
/// [`MachineApplicable`]: Applicability::MachineApplicable
/// [`MaybeIncorrect`]: Applicability::MaybeIncorrect
pub(crate) fn emit_flagged_char<Cx>(
    lint_context: &Cx,
    lint: &'static Lint,
    character: char,
    span: Span,
    context_label: &str,
) where
    Cx: LintContext,
{
    let applicability = if character == '\u{2026}' {
        Applicability::MachineApplicable
    } else {
        Applicability::MaybeIncorrect
    };
    span_lint_and_sugg(
        lint_context,
        lint,
        span,
        format!(
            "Unicode `{character}` (U+{:04X}) in {context_label}",
            character as u32,
        ),
        "use ASCII `...` instead",
        "...".to_owned(),
        applicability,
    );
}

/// Return `(prefix_length, suffix_length)` covering the opening and
/// closing delimiters of a Rust string-literal snippet, or `None` if
/// the snippet does not look like a string literal whose body we can
/// scan as plain text.
///
/// Recognises plain (`"..."`) and raw (`r"..."`, `r#"..."#`, ...)
/// strings. Byte / C-string forms are excluded — the helper is for
/// rules that operate on display strings.
pub(crate) fn string_literal_quote_lengths(snippet: &str) -> Option<(usize, usize)> {
    let bytes = snippet.as_bytes();
    let mut index = 0;
    let mut hash_count = 0;
    if index < bytes.len() && bytes[index] == b'r' {
        index += 1;
        while index < bytes.len() && bytes[index] == b'#' {
            hash_count += 1;
            index += 1;
        }
    }
    if index >= bytes.len() || bytes[index] != b'"' {
        return None;
    }
    let prefix_length = index + 1;
    let expected_suffix_length = hash_count + 1;
    if bytes.len() < prefix_length + expected_suffix_length {
        return None;
    }
    let suffix_start = bytes.len() - expected_suffix_length;
    if bytes[suffix_start] != b'"' {
        return None;
    }
    for trailing_hash_index in 0..hash_count {
        if bytes[suffix_start + 1 + trailing_hash_index] != b'#' {
            return None;
        }
    }
    Some((prefix_length, expected_suffix_length))
}
