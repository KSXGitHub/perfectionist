//! Source-text scanner for the trailing `// ...` line comment after
//! a lint-level attribute, plus the text-normalisation that turns a
//! raw comment into the rationale-string body that ends up inside
//! `reason = "..."`.

/// Trailing comment located by [`find_trailing_comment`]. Byte
/// offsets are relative to the source file's start (i.e. inside
/// `SourceFile::src`).
pub(super) struct Comment {
    /// Normalised text to put inside the `reason = "..."` literal.
    pub(super) text: String,
    /// Range of bytes whose removal makes the comment disappear from
    /// source: the run of horizontal whitespace after the closing
    /// `]` plus the `// ...` text. Excludes the line terminator —
    /// including a trailing `\r` of a `\r\n` ending — so applying the
    /// fix never rewrites the line's ending.
    pub(super) delete_start: usize,
    pub(super) delete_end: usize,
    /// Range of bytes covering the comment text proper (`//` through
    /// to the end of the comment). Used as the diagnostic's primary
    /// span.
    pub(super) diag_start: usize,
    pub(super) diag_end: usize,
}

/// Scan forward from the attribute's closing `]` for a trailing
/// comment on the same source line. Returns `None` if there is no
/// `//` comment between `]` and the next newline, if the only
/// content there is a doc-comment marker (`///`, `//!`), or if the
/// matched comment normalises to empty (a bare `//`, whitespace, or
/// an all-decoration divider) — an empty comment carries no
/// rationale worth lifting.
pub(super) fn find_trailing_comment(source: &str, attr_hi: usize) -> Option<Comment> {
    let bytes = source.as_bytes();
    let mut cursor = attr_hi;
    while cursor < bytes.len() && is_horizontal_whitespace(bytes[cursor]) {
        cursor += 1;
    }
    if cursor + 2 > bytes.len() || &bytes[cursor..cursor + 2] != b"//" {
        return None;
    }
    if is_doc_comment_prefix(&bytes[cursor + 2..]) {
        return None;
    }
    let comment_start = cursor;
    let mut end = comment_start;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    // A line that ends `\r\n` has a `\r` immediately before the
    // newline; strip it so the lifted text doesn't carry a stray
    // carriage return.
    let text_end = if end > comment_start && bytes[end - 1] == b'\r' {
        end - 1
    } else {
        end
    };
    let text = normalise_comment_text(&source[comment_start..text_end]);
    if text.is_empty() {
        return None;
    }
    Some(Comment {
        text,
        delete_start: attr_hi,
        // `text_end`, not `end`: for a `\r\n` ending `end` points at
        // the `\n` and the deletion range `[attr_hi, end)` would
        // include the preceding `\r`, silently rewriting that line's
        // ending to LF. Stopping at `text_end` deletes only the
        // whitespace + comment and leaves the `\r\n` intact.
        delete_end: text_end,
        diag_start: comment_start,
        diag_end: text_end,
    })
}

fn is_horizontal_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

/// Whether the bytes immediately *after* a `//` open the comment as
/// a doc comment rather than a regular comment.
///
/// rustc treats `//!` as an inner doc comment and a *three-slash*
/// `///` (third slash not followed by another `/`) as an outer doc
/// comment. A `////` run with four or more slashes is a regular
/// comment again — the same classification rustc_lexer uses.
fn is_doc_comment_prefix(rest_after_slashes: &[u8]) -> bool {
    match rest_after_slashes {
        [b'!', ..] => true,
        [b'/'] => true,
        [b'/', next, ..] if *next != b'/' => true,
        _ => false,
    }
}

/// Strip the `//` marker, trim ASCII whitespace, and strip a leading
/// run of decoration characters (`-`, `=`, `*`) followed by
/// whitespace.
///
/// Returns the empty string for inputs that carry no rationale text:
/// a bare `//`, a whitespace-only `//   `, or an all-decoration
/// visual divider like `//----------`. [`find_trailing_comment`]
/// uses the empty return to skip those matches rather than lift a
/// vacuous reason.
///
/// The input `content` is expected to start with `//`. It is not the
/// raw line — trailing `\r` and similar line-terminator bytes are
/// already removed by the caller.
fn normalise_comment_text(content: &str) -> String {
    let after_slashes = content.strip_prefix("//").unwrap_or(content);
    let trimmed = after_slashes.trim_matches([' ', '\t', '\r']);
    let bytes = trimmed.as_bytes();
    let mut run = 0;
    while run < bytes.len() && matches!(bytes[run], b'-' | b'=' | b'*') {
        run += 1;
    }
    if run > 0 {
        // A run that fills the entire trimmed slice is an
        // all-decoration line — `//-----------` or `//=== ` (the
        // trailing whitespace was already stripped by `trim_matches`).
        // It carries no rationale, so return empty and let
        // `find_trailing_comment` treat it as a no-match.
        if run == bytes.len() {
            return String::new();
        }
        if bytes
            .get(run)
            .is_some_and(|byte| is_horizontal_whitespace(*byte))
        {
            let after = &trimmed[run..];
            return after.trim_start_matches([' ', '\t']).to_owned();
        }
    }
    trimmed.to_owned()
}

#[cfg(test)]
mod tests;
