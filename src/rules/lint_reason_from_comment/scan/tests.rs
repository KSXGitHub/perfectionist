use super::{Comment, find_trailing_comment, normalise_comment_text};

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
/// empty string, which [`find_trailing_comment`] uses to skip the
/// match rather than lift a vacuous reason.
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
/// only, all-decoration divider) returns `None` so the rule does
/// not lift a vacuous `reason = ""`.
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

/// On a `\r\n` line the deletion range must stop before the `\r`,
/// so applying the fix removes only the comment and leaves the
/// `\r\n` ending intact (rather than silently flipping it to LF).
#[test]
fn trailing_crlf_delete_range_excludes_carriage_return() {
    let source = "#[allow(foo)] // hello\r\n";
    let attr_hi = source.find(']').unwrap() + 1;
    let comment = find_trailing_comment(source, attr_hi).expect("expected a match");
    // The byte at `delete_end` is the `\r`; it (and the `\n`) must
    // survive the deletion.
    assert_eq!(&source[comment.delete_end..], "\r\n");
}
