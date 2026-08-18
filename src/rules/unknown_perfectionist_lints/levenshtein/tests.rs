use super::{Candidate, levenshtein};

/// The registered lint name the typo fixtures below aim at.
const REGISTERED: &str = "unicode_ellipsis_in_comments";

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
    // Insertion, deletion, substitution, transposition.
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
fn ascii_candidate_measures_bytes() {
    let candidate = Candidate::new("unicode_ellipsis_in_comment");
    assert!(matches!(candidate, Candidate::Ascii(_)));
    assert_eq!(candidate.distance_to(REGISTERED), 1);
    assert_eq!(candidate.distance_to("unordered_derives"), 20);
}

#[test]
fn non_ascii_candidate_measures_characters() {
    // Cyrillic `о` for ASCII `o`: one character, but two bytes.
    let cyrillic = Candidate::new("unicode_ellipsis_in_cоmments");
    assert!(matches!(cyrillic, Candidate::Unicode(_)));
    assert_eq!(cyrillic.distance_to(REGISTERED), 1);

    // Fullwidth `ｏ` for ASCII `o`: one character, but three bytes —
    // over the default `suggestion_distance` had bytes been compared.
    let fullwidth = Candidate::new("unicode_ellipsis_in_cｏmments");
    assert_eq!(fullwidth.distance_to(REGISTERED), 1);
}

/// The character-wise path exists because the byte-wise one overcounts
/// a multi-byte character. Pin that difference down, so the fallback
/// cannot be dropped as redundant.
#[test]
fn bytes_overcount_multi_byte_characters() {
    assert_eq!(
        levenshtein(
            "unicode_ellipsis_in_cоmments".as_bytes(),
            REGISTERED.as_bytes()
        ),
        2,
    );
    assert_eq!(
        levenshtein(
            "unicode_ellipsis_in_cｏmments".as_bytes(),
            REGISTERED.as_bytes()
        ),
        3,
    );
}

/// Both element types describe the same distance whenever the candidate
/// is ASCII — the property that lets the common path skip decoding.
#[test]
fn byte_and_character_paths_agree_on_ascii() {
    let pairs = [
        ("unicode_ellipsis_in_comment", REGISTERED),
        ("unknown_perfectionist_lint", "unknown_perfectionist_lints"),
        ("nothing_like_this_anywhere", REGISTERED),
        ("", REGISTERED),
    ];
    for (candidate, registered) in pairs {
        let bytes = levenshtein(candidate.as_bytes(), registered.as_bytes());
        let characters = levenshtein(
            &candidate.chars().collect::<Vec<char>>(),
            &registered.chars().collect::<Vec<char>>(),
        );
        assert_eq!(bytes, characters, "{candidate} vs {registered}");
    }
}
