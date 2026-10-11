//! The lexical walk that finds every numeric escape of a visible
//! character in the crate's own source.
//!
//! The rule is a question about source spelling, so the walk is a
//! `rustc_lexer` pass over each source file rather than a HIR walk.
//! That reaches a literal a macro consumes during expansion (a
//! `format!` template carrying a placeholder is split into its static
//! pieces, `stringify!` reflects its argument as text) and a literal
//! behind a `#[cfg(...)]` that is not part of the build, neither of
//! which survives into the HIR as a literal node.
//!
//! Only a file backing a module of the crate's HIR module tree is
//! walked, so a data file pulled in with `include_str!` is skipped:
//! its contents are not Rust, and a quoted string inside it lexes as
//! a Rust literal all the same. The sibling text-scanning rules share
//! that filter for the same reason.

use super::visibility::is_visible;
use crate::literal_scan::take_string_escape;
use crate::module_reparse::crate_module_files;
use rustc_lexer::{FrontmatterAllowed, LiteralKind, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::{BytePos, Pos, RelativeBytePos, SourceFile, Span, SyntaxContext};

/// One flagged escape, carried to the emitter alongside its span.
pub(super) struct Violation {
    /// The character the escape stands for, and the replacement the
    /// suggestion offers.
    pub(super) character: char,
    /// The escape's source spelling, quoted back in the diagnostic so
    /// the message names the form that was written.
    pub(super) escape: String,
}

/// Walk every source file backing a module of the local crate and
/// collect one entry per numeric escape of a visible character.
pub(super) fn collect_violations(lint_context: &LateContext<'_>) -> Vec<(Span, Violation)> {
    let module_files = crate_module_files(lint_context);
    let source_map = lint_context.sess().source_map();
    let mut violations: Vec<(Span, Violation)> = Vec::new();
    for source_file in source_map.files().iter() {
        if source_file.cnum != LOCAL_CRATE {
            continue;
        }
        if !module_files.contains(&source_file.name) {
            continue;
        }
        let Some(source_text) = source_file.src.as_deref() else {
            continue;
        };
        scan_source_file(source_text, source_file, &mut violations);
    }
    violations
}

fn scan_source_file(source_text: &str, source_file: &SourceFile, out: &mut Vec<(Span, Violation)>) {
    let mut offset: u32 = 0;
    for token in tokenize(source_text, FrontmatterAllowed::Yes) {
        let end = offset
            .checked_add(token.len)
            .expect("source-file offset overflowed u32");
        // A suffix on a text literal is invalid but still lexes, and
        // `suffix_start` is where it begins, so cutting there leaves the
        // literal proper — whose last byte is its closing delimiter.
        if let TokenKind::Literal { kind, suffix_start } = token.kind
            && let Some(literal_end) = offset.checked_add(suffix_start)
            && let Some(literal) = source_text.get(offset as usize..literal_end as usize)
            && let Some(text) = TextLiteral::parse(kind, literal)
        {
            scan_literal_body(&text, offset, source_file, out);
        }
        offset = end;
    }
}

/// A literal whose contents are text rather than bytes, split into
/// the part the escapes live in and the delimiter that part cannot
/// hold verbatim.
///
/// A raw literal carries no escapes, an unterminated one is broken
/// source, and a byte literal holds bytes rather than characters, so
/// [`TextLiteral::parse`] answers `None` for each.
struct TextLiteral<'a> {
    /// The literal's contents, between its delimiters.
    body: &'a str,
    /// Byte offset of `body` within the literal.
    body_offset: u32,
    /// The closing delimiter: `"` for a string or C-string literal,
    /// `'` for a character literal.
    delimiter: char,
}

impl<'a> TextLiteral<'a> {
    fn parse(kind: LiteralKind, literal: &'a str) -> Option<Self> {
        let (prefix, delimiter) = match kind {
            LiteralKind::Str { terminated: true } => (r#"""#, '"'),
            LiteralKind::CStr { terminated: true } => (r#"c""#, '"'),
            LiteralKind::Char { terminated: true } => ("'", '\''),
            _ => return None,
        };
        let body = literal.strip_prefix(prefix)?.strip_suffix(delimiter)?;
        Some(Self {
            body,
            body_offset: prefix.len() as u32,
            delimiter,
        })
    }
}

/// Scan one literal's body, pushing an entry per numeric escape whose
/// character is visible and spellable in this literal.
///
/// `literal_offset` is the literal's start within the source file,
/// which every escape's span is measured from.
fn scan_literal_body(
    text: &TextLiteral<'_>,
    literal_offset: u32,
    source_file: &SourceFile,
    out: &mut Vec<(Span, Violation)>,
) {
    let mut rest = text.body;
    let mut offset_in_body: usize = 0;
    while let Some(index) = rest.find('\\') {
        offset_in_body += index;
        rest = &rest[index..];
        // A malformed escape means source that will not compile, so
        // the rest of this literal is not worth reading: scanning past
        // it would misread the bytes that follow as plain text.
        let Some((escape, remainder)) = take_string_escape(rest) else {
            return;
        };
        if let Some(character) = decode_numeric_escape(escape)
            && is_visible(character)
            // The delimiter and the backslash have no verbatim
            // spelling inside the literal, so there is nothing to
            // suggest in their place.
            && character != text.delimiter
            && character != '\\'
        {
            let start = source_file.absolute_position(RelativeBytePos::from_u32(
                literal_offset + text.body_offset + offset_in_body as u32,
            ));
            let end = BytePos::from_u32(start.0 + escape.len() as u32);
            out.push((
                Span::new(start, end, SyntaxContext::root(), None),
                Violation {
                    character,
                    escape: escape.to_owned(),
                },
            ));
        }
        offset_in_body += escape.len();
        rest = remainder;
    }
}

/// The character a numeric escape stands for, or `None` for any other
/// escape (`\n`, `\t`, `\0`, `\"`, `\\`, a line continuation) and for
/// a numeric escape that does not denote one character.
///
/// `\xNN` above `\x7F` answers `None`: a string or character literal
/// rejects it outright, and in a C-string literal it denotes that one
/// byte, which the matching character would encode as two.
fn decode_numeric_escape(escape: &str) -> Option<char> {
    if let Some(digits) = escape.strip_prefix(r"\x") {
        let byte = u8::from_str_radix(digits, 16).ok()?;
        return byte.is_ascii().then(|| char::from(byte));
    }
    let digits = escape.strip_prefix(r"\u{")?.strip_suffix('}')?;
    // The grammar allows `_` between the hex digits of a `\u{...}`
    // escape, as it does in a numeric literal.
    let codepoint = u32::from_str_radix(&digits.replace('_', ""), 16).ok()?;
    char::from_u32(codepoint)
}

#[cfg(test)]
mod tests;
