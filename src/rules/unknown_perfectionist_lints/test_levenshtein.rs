use super::levenshtein;

#[test]
fn empty_sides_cost_the_other_length() {
    assert_eq!(levenshtein(b"", b""), 0);
    assert_eq!(levenshtein(b"", b"lint"), 4);
    assert_eq!(levenshtein(b"lint", b""), 4);
}

#[test]
fn textbook_distances() {
    assert_eq!(levenshtein(b"kitten", b"sitting"), 3);
    assert_eq!(levenshtein(b"lint", b"lint"), 0);
    assert_eq!(levenshtein(b"lint", b"lints"), 1);
    assert_eq!(levenshtein(b"lints", b"lint"), 1);
    assert_eq!(levenshtein(b"lint", b"link"), 1);
    assert_eq!(levenshtein(b"form", b"from"), 2);
}

#[test]
fn distance_is_symmetric() {
    assert_eq!(levenshtein(b"wildcard_imports", b"wildcard_import"), 1);
    assert_eq!(levenshtein(b"wildcard_import", b"wildcard_imports"), 1);
}

#[test]
fn lint_name_typos() {
    assert_eq!(
        levenshtein(
            b"unicode_ellipsis_in_comment",
            b"unicode_ellipsis_in_comments",
        ),
        1,
    );
    // A dropped word fragment — the case that sets `SUGGESTION_DISTANCE`.
    assert_eq!(
        levenshtein(
            b"unicode_ellipsis_comments",
            b"unicode_ellipsis_in_comments",
        ),
        3,
    );
    assert_eq!(
        levenshtein(
            b"nothing_like_this_anywhere",
            b"unicode_ellipsis_in_comments",
        ),
        23,
    );
}
