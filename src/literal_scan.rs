//! Helpers shared by rules that scan string-literal / comment text.
//!
//! [`emit_flagged_chars`] serves a rule that has a contiguous stretch
//! of text to sweep: walk it, emit a diagnostic for each flagged
//! character, and offer the same `...` autofix. The per-character
//! logic is identical across the Unicode-ellipsis rules; the only
//! per-rule pieces are the lint name, a context label, and how to turn
//! a byte offset within the text into a [`Span`].
//!
//! [`emit_flagged_char_hir`] emits one character's diagnostic at a
//! caller-supplied [`HirId`], for a rule that runs its own scan loop —
//! one that must consult a markdown code-region mask and a fallible
//! span map before emitting, say. It shares the message, suggestion,
//! and applicability with the sweep above rather than restating
//! them.
//!
//! [`string_literal_quote_lengths`] is the companion parser for any
//! rule that needs to scan a string-literal body without its opening
//! and closing delimiters. It sits here rather than inside any one
//! rule because the shape it recognises (plain and raw display
//! strings) is a generic property of Rust string literals, not
//! specific to any rule's subject.
//!
//! [`take_string_escape`] is the escape-aware front-of-body scanner
//! shared by every rule that walks a *cooked* string literal's body
//! and must tell a real backslash escape (`\n`, `\\`, `\xNN`,
//! `\u{...}`, a line continuation, ...) apart from the bytes around
//! it. A caller uses that either to bail on the first non-eligible
//! escape, or to find the real `\n` escapes in a literal it is about
//! to fold without being fooled by `\\n` (an escaped backslash
//! followed by the letter `n`, which is *not* a newline).

use clippy_utils::diagnostics::{span_lint_and_sugg, span_lint_hir_and_then};
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::{LateContext, Lint, LintContext};
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

/// Label on the `...` suggestion. The two emitters below reach
/// different `clippy_utils` entry points and assemble the diagnostic
/// separately; naming the label pins the one part of it that is a
/// bare literal in both.
const SUGGESTION_LABEL: &str = "use ASCII `...` instead";

/// Replacement text the suggestion offers, named for the same reason
/// as [`SUGGESTION_LABEL`].
const ASCII_ELLIPSIS: &str = "...";

/// Emit a single flagged-character diagnostic at `span`, suggesting
/// the ASCII `...` replacement. The body of [`emit_flagged_chars`]'
/// loop, split out so the [`HirId`]-anchored
/// [`emit_flagged_char_hir`] can be checked against it at a glance.
///
/// Applicability is [`MachineApplicable`] for U+2026 (the rules'
/// primary target, which always maps cleanly to `...`) and
/// [`MaybeIncorrect`] for any user-configured `extra_flagged_chars`
/// entry (whose visual equivalence to `...` is up to the project to
/// assert).
///
/// [`MachineApplicable`]: Applicability::MachineApplicable
/// [`MaybeIncorrect`]: Applicability::MaybeIncorrect
fn emit_flagged_char<Cx>(
    lint_context: &Cx,
    lint: &'static Lint,
    character: char,
    span: Span,
    context_label: &str,
) where
    Cx: LintContext,
{
    span_lint_and_sugg(
        lint_context,
        lint,
        span,
        flagged_char_message(character, context_label),
        SUGGESTION_LABEL,
        ASCII_ELLIPSIS.to_owned(),
        flagged_char_applicability(character),
    );
}

/// HIR-anchored counterpart of [`emit_flagged_char`], for a rule that
/// runs in a late pass and emits at the enclosing HIR node of the
/// comment it scanned — resolved by
/// [`crate::enclosing_hir::emit_at_enclosing_hir`] — so a per-item /
/// per-module `#[allow]` / `#[expect]` resolves, not just a crate-root
/// `#![allow]`. It shares [`flagged_char_message`],
/// [`flagged_char_applicability`], [`SUGGESTION_LABEL`] and
/// [`ASCII_ELLIPSIS`] with [`emit_flagged_char`]; anything added to
/// the diagnostic beyond those has to be added in both places.
pub(crate) fn emit_flagged_char_hir(
    lint_context: &LateContext<'_>,
    lint: &'static Lint,
    hir_id: HirId,
    character: char,
    span: Span,
    context_label: &str,
) {
    let applicability = flagged_char_applicability(character);
    span_lint_hir_and_then(
        lint_context,
        lint,
        hir_id,
        span,
        flagged_char_message(character, context_label),
        |diag| {
            diag.span_suggestion(span, SUGGESTION_LABEL, ASCII_ELLIPSIS, applicability);
        },
    );
}

/// Diagnostic message for a flagged character, shared by the
/// current-context [`emit_flagged_char`] and HIR-anchored
/// [`emit_flagged_char_hir`] emitters so the two stay identical.
fn flagged_char_message(character: char, context_label: &str) -> String {
    format!(
        "Unicode `{character}` (U+{:04X}) in {context_label}",
        character as u32,
    )
}

/// Suggestion applicability for a flagged character: `MachineApplicable`
/// for U+2026 (always maps cleanly to `...`), `MaybeIncorrect` for any
/// user-configured `extra_flagged_chars` entry.
fn flagged_char_applicability(character: char) -> Applicability {
    if character == '\u{2026}' {
        Applicability::MachineApplicable
    } else {
        Applicability::MaybeIncorrect
    }
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

/// Take a single backslash escape from the front of a *cooked*
/// string-literal body and return `(escape_text, remainder)`, or
/// `None` if `input` does not start with `\` or the escape is
/// malformed (incomplete `\u{...}` without a closing brace, dangling
/// backslash at end of input, truncated `\xNN`).
///
/// Recognises `\xNN` (4 bytes), `\u{...}` (variable length), and any
/// single-character escape (`\n`, `\t`, `\r`, `\0`, `\"`, `\\`, `\'`,
/// the line-continuation `\<newline>`, ...). The returned slice is the
/// verbatim source spelling of the escape, escapes are not decoded —
/// callers that need the decoded character derive it from the slice.
///
/// The "cooked" qualifier is load-bearing: in a *raw* string literal a
/// backslash is an ordinary character, so running this scanner over a
/// raw body would misread `r"\n"` (literal backslash, literal `n`) as
/// a newline escape. Callers must gate on the literal being cooked
/// before scanning its body with this function.
pub(crate) fn take_string_escape(input: &str) -> Option<(&str, &str)> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'\\') {
        return None;
    }
    let second_byte = *bytes.get(1)?;
    let escape_len = match second_byte {
        b'x' => 4,
        b'u' => {
            // `\u{...}`: scan to the closing `}`. The bytes between
            // `{` and `}` are constrained to ASCII hex by the rustc
            // lexer, so a byte-level scan is sufficient.
            let mut length: usize = 2;
            let mut closing_found = false;
            for &byte in &bytes[2..] {
                length = length.saturating_add(1);
                if byte == b'}' {
                    closing_found = true;
                    break;
                }
            }
            if !closing_found {
                return None;
            }
            length
        }
        _ => {
            // `\` + a single UTF-8 character (e.g. `\n`, `\"`,
            // or the line-continuation `\<newline>`).
            let second_char = input['\\'.len_utf8()..].chars().next()?;
            '\\'.len_utf8() + second_char.len_utf8()
        }
    };
    if escape_len > input.len() {
        return None;
    }
    Some(input.split_at(escape_len))
}
