//! Source-text scanners for adjacent `// ...` line comments and the
//! shared text-normalisation that turns a raw comment into the
//! rationale-string body that ends up inside `reason = "..."`.

/// Adjacent comment located by [`find_trailing_comment`] or
/// [`find_leading_comment`]. Byte offsets are relative to the source
/// file's start (i.e. inside `SourceFile::src`).
pub(super) struct Comment {
    /// Normalised text to put inside the `reason = "..."` literal.
    pub(super) text: String,
    /// Range of bytes whose removal makes the comment disappear from
    /// source. For a trailing comment this is the run of horizontal
    /// whitespace + the `// ...` text (not the newline that
    /// terminates the line). For a leading comment this is the whole
    /// line including its trailing line terminator, so removing it
    /// leaves no blank line behind.
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
/// an all-decoration divider) — the empty case lets `check` fall
/// through to the leading-comment placement instead of pre-empting
/// it with a vacuous trailing match.
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
        delete_end: end,
        diag_start: comment_start,
        diag_end: text_end,
    })
}

/// Scan backward from the attribute's opening `#` for a `// ...`
/// comment on the immediately preceding source line. Returns `None`
/// unless the previous line consists *only* of a regular line
/// comment (after any leading indentation), is separated from the
/// attribute by exactly one line terminator, and normalises to
/// non-empty text.
pub(super) fn find_leading_comment(source: &str, attr_lo: usize) -> Option<Comment> {
    let bytes = source.as_bytes();
    if attr_lo > bytes.len() {
        return None;
    }
    // Walk back over the attribute's indentation on its own line.
    let mut cursor = attr_lo;
    while cursor > 0 && is_horizontal_whitespace(bytes[cursor - 1]) {
        cursor -= 1;
    }
    // The attribute must start at the beginning of a line — the byte
    // immediately before the run of horizontal whitespace must be a
    // newline. A non-newline (or start-of-file) means the attribute
    // shares a line with earlier content (`#[other_attr] #[allow]`
    // chains, an inner attribute mid-expression), in which case a
    // leading comment is not meaningful.
    if cursor == 0 || bytes[cursor - 1] != b'\n' {
        return None;
    }
    let attr_line_start = cursor;
    // The `\n` is at `attr_line_start - 1`. Step past it and any
    // immediately-preceding `\r` so the deletion range covers the
    // whole line terminator.
    let newline_pos = attr_line_start - 1;
    let prev_line_terminator_start = if newline_pos > 0 && bytes[newline_pos - 1] == b'\r' {
        newline_pos - 1
    } else {
        newline_pos
    };
    // Walk back to the start of the previous line.
    let mut prev_line_start = prev_line_terminator_start;
    while prev_line_start > 0 && bytes[prev_line_start - 1] != b'\n' {
        prev_line_start -= 1;
    }
    // Skip the previous line's own indentation.
    let mut comment_start = prev_line_start;
    while comment_start < prev_line_terminator_start
        && is_horizontal_whitespace(bytes[comment_start])
    {
        comment_start += 1;
    }
    if comment_start + 2 > prev_line_terminator_start
        || &bytes[comment_start..comment_start + 2] != b"//"
    {
        return None;
    }
    if is_doc_comment_prefix(&bytes[comment_start + 2..prev_line_terminator_start]) {
        return None;
    }
    let text = normalise_comment_text(&source[comment_start..prev_line_terminator_start]);
    if text.is_empty() {
        return None;
    }
    Some(Comment {
        text,
        delete_start: prev_line_start,
        delete_end: attr_line_start,
        diag_start: comment_start,
        diag_end: prev_line_terminator_start,
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
/// visual divider like `//----------`. [`find_trailing_comment`] /
/// [`find_leading_comment`] use the empty return to skip those
/// matches so `check` falls through to the next placement instead
/// of lifting a vacuous reason.
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
        // `find_*_comment` treat it as a no-match.
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
mod tests {
    use super::{Comment, find_leading_comment, find_trailing_comment, normalise_comment_text};

    #[test]
    fn normalise_strips_markers_and_decoration() {
        assert_eq!(normalise_comment_text("// hello"), "hello");
        assert_eq!(normalise_comment_text("//   hello  "), "hello");
        assert_eq!(normalise_comment_text("//-- hello"), "hello");
        assert_eq!(normalise_comment_text("//== hello"), "hello");
        assert_eq!(normalise_comment_text("//* hello"), "hello");
        // Decoration without trailing whitespace passes through.
        assert_eq!(normalise_comment_text("//--hello"), "--hello");
        // A non-recognised decoration prefix passes through.
        assert_eq!(normalise_comment_text("// > quoted"), "> quoted");
    }

    /// A bare `//` or whitespace-only `//   ` line normalises to an
    /// empty string, which `find_*_comment` use to skip the match so
    /// `check` falls through to the next placement.
    #[test]
    fn normalise_collapses_empty_and_whitespace_only_comments() {
        assert_eq!(normalise_comment_text("//"), "");
        assert_eq!(normalise_comment_text("//   "), "");
        assert_eq!(normalise_comment_text("//\t"), "");
    }

    /// All-decoration lines (visual dividers) normalise to empty so
    /// they don't get lifted as a vacuous `reason = "==="`-style
    /// rationale.
    #[test]
    fn normalise_collapses_all_decoration_comments() {
        assert_eq!(normalise_comment_text("//----------"), "");
        assert_eq!(normalise_comment_text("//=========="), "");
        assert_eq!(normalise_comment_text("//**********"), "");
        // Mixed decoration runs collapse the same way.
        assert_eq!(normalise_comment_text("//-=-=-=-="), "");
        // Trailing whitespace was already trimmed, so a divider with
        // trailing space behaves identically.
        assert_eq!(normalise_comment_text("//---   "), "");
    }

    fn assert_comment(actual: Option<Comment>, expected_text: &str) {
        let comment = actual.expect("expected a comment match");
        assert_eq!(comment.text, expected_text);
    }

    #[test]
    fn trailing_simple() {
        let source = "#[allow(foo)] // hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert_comment(find_trailing_comment(source, attr_hi), "hello");
    }

    #[test]
    fn trailing_skips_doc_marker() {
        let source = "#[allow(foo)] /// hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert!(find_trailing_comment(source, attr_hi).is_none());
    }

    #[test]
    fn trailing_skips_inner_doc_marker() {
        let source = "#[allow(foo)] //! hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert!(find_trailing_comment(source, attr_hi).is_none());
    }

    /// An empty-normalised trailing comment (bare `//`, whitespace
    /// only, all-decoration divider) returns `None` so `check` can
    /// fall through to the leading-comment placement instead of
    /// short-circuiting with a vacuous trailing match.
    #[test]
    fn trailing_empty_normalised_returns_none() {
        for source in [
            "#[allow(foo)] //\n",
            "#[allow(foo)] //   \n",
            "#[allow(foo)] //----------\n",
        ] {
            let attr_hi = source.find(']').unwrap() + 1;
            assert!(
                find_trailing_comment(source, attr_hi).is_none(),
                "expected no match for {source:?}",
            );
        }
    }

    /// Symmetric to `trailing_empty_normalised_returns_none`: a
    /// leading line that normalises to empty doesn't lift either.
    #[test]
    fn leading_empty_normalised_returns_none() {
        for source in [
            "//\n#[allow(foo)]\n",
            "//   \n#[allow(foo)]\n",
            "//----------\n#[allow(foo)]\n",
        ] {
            let attr_lo = source.find('#').unwrap();
            assert!(
                find_leading_comment(source, attr_lo).is_none(),
                "expected no match for {source:?}",
            );
        }
    }

    #[test]
    fn trailing_accepts_quadruple_slash() {
        let source = "#[allow(foo)] //// hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        // `////` is a regular comment in rustc_lexer's classification;
        // the rule lifts its text minus the `//` marker. Normalisation
        // doesn't strip extra slashes; that's left for the author to
        // tidy up after the autofix.
        assert_comment(find_trailing_comment(source, attr_hi), "// hello");
    }

    #[test]
    fn trailing_no_comment_on_same_line() {
        let source = "#[allow(foo)]\n// next line\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert!(find_trailing_comment(source, attr_hi).is_none());
    }

    #[test]
    fn trailing_handles_crlf() {
        let source = "#[allow(foo)] // hello\r\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert_comment(find_trailing_comment(source, attr_hi), "hello");
    }

    #[test]
    fn leading_simple() {
        let source = "// hello\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert_comment(find_leading_comment(source, attr_lo), "hello");
    }

    #[test]
    fn leading_with_indentation() {
        let source = "    // hello\n    #[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert_comment(find_leading_comment(source, attr_lo), "hello");
    }

    #[test]
    fn leading_skips_doc_marker() {
        let source = "/// hello\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }

    #[test]
    fn leading_rejects_blank_line_between() {
        let source = "// hello\n\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }

    #[test]
    fn leading_rejects_other_attribute_on_prev_line() {
        let source = "#[other]\n#[allow(foo)]\n";
        let attr_lo = source.rfind('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }

    #[test]
    fn leading_rejects_when_prev_line_has_other_content() {
        let source = "let x = 1; // x\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }
}
