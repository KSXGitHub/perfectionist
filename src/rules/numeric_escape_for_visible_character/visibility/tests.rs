use super::{Scope, is_visible};

/// The scope the rule has when `dylint.toml` says nothing about it,
/// where a character's own visibility is the whole test.
const EVERY_VISIBLE_CHARACTER: Scope = Scope {
    flag_non_ascii: true,
};

/// The scope a consumer running `clippy::non_ascii_literal` alongside
/// this rule asks for.
const ASCII_ONLY: Scope = Scope {
    flag_non_ascii: false,
};

/// Every codepoint a diagnostic should fire on, paired with the name
/// that says why it is visible. Spelled as codepoints so the test
/// reads as a table of numbers rather than of glyphs, several of which
/// no editor font would distinguish.
const VISIBLE: &[(u32, &str)] = &[
    (0x21, "EXCLAMATION MARK"),
    (0x41, "LATIN CAPITAL LETTER A"),
    (0x7B, "LEFT CURLY BRACKET"),
    (0x7E, "TILDE"),
    (0xE9, "LATIN SMALL LETTER E WITH ACUTE"),
    (0x2026, "HORIZONTAL ELLIPSIS"),
    (0x2192, "RIGHTWARDS ARROW"),
    (0x2715, "MULTIPLICATION X"),
    (0x4E00, "CJK UNIFIED IDEOGRAPH-4E00"),
    (0x1F600, "GRINNING FACE"),
];

/// Every codepoint the rule leaves escaped, paired with the family it
/// belongs to. Each measures zero columns or none at all, is
/// whitespace, or is private use.
const INVISIBLE: &[(u32, &str)] = &[
    (0x0, "NULL, a control character"),
    (
        0x9,
        "CHARACTER TABULATION, whitespace and a control character",
    ),
    (0xA, "LINE FEED, whitespace and a control character"),
    (0x1B, "ESCAPE, a control character"),
    (0x20, "SPACE, whitespace"),
    (0x7F, "DELETE, a control character"),
    (0x85, "NEXT LINE, a C1 control character"),
    (0xA0, "NO-BREAK SPACE, whitespace"),
    (0xAD, "SOFT HYPHEN, a format character"),
    (0x61C, "ARABIC LETTER MARK, a format character"),
    (0x301, "COMBINING ACUTE ACCENT, a combining mark"),
    (0x200B, "ZERO WIDTH SPACE"),
    (0x200D, "ZERO WIDTH JOINER, a format character"),
    (0x202E, "RIGHT-TO-LEFT OVERRIDE, a bidi control"),
    (0x2028, "LINE SEPARATOR, whitespace"),
    (0x2060, "WORD JOINER, a format character"),
    (0x20E3, "COMBINING ENCLOSING KEYCAP, an enclosing mark"),
    (0x3000, "IDEOGRAPHIC SPACE, whitespace"),
    (0xFE0F, "VARIATION SELECTOR-16"),
    (0xFEFF, "ZERO WIDTH NO-BREAK SPACE, the byte-order mark"),
    (0xE000, "the first Private Use Area codepoint"),
    (0xE0B0, "a Private Use Area codepoint an icon font assigns"),
    (0xF8FF, "the last Private Use Area codepoint in the BMP"),
    (0xE0041, "TAG LATIN CAPITAL LETTER A, a format character"),
    (0xF0000, "the first codepoint of Plane 15, all private use"),
    (0x10FFFD, "the last codepoint of Plane 16, all private use"),
];

fn character(codepoint: u32) -> char {
    char::from_u32(codepoint).expect("the table holds no surrogate")
}

/// Both tables at once, for a test whose claim covers every row of
/// each.
fn every_row() -> impl Iterator<Item = (u32, &'static str)> {
    VISIBLE.iter().chain(INVISIBLE).copied()
}

#[test]
fn visible_characters_are_spelled_out() {
    for &(codepoint, name) in VISIBLE {
        assert!(
            is_visible(character(codepoint)),
            "U+{codepoint:04X} {name} should be flagged",
        );
    }
}

#[test]
fn invisible_characters_keep_their_escape() {
    for &(codepoint, name) in INVISIBLE {
        assert!(
            !is_visible(character(codepoint)),
            "U+{codepoint:04X} {name} should not be flagged",
        );
    }
}

#[test]
fn the_default_scope_asks_only_whether_the_character_is_visible() {
    for (codepoint, name) in every_row() {
        let character = character(codepoint);
        assert_eq!(
            EVERY_VISIBLE_CHARACTER.covers(character),
            is_visible(character),
            "U+{codepoint:04X} {name} is covered by default exactly when it is visible",
        );
    }
}

#[test]
fn narrowing_to_ascii_drops_the_non_ascii_characters() {
    for (codepoint, name) in every_row() {
        let character = character(codepoint);
        assert_eq!(
            ASCII_ONLY.covers(character),
            is_visible(character) && character.is_ascii(),
            "U+{codepoint:04X} {name} is covered under `flag_non_ascii = false` \
             exactly when it is both visible and ASCII",
        );
    }
}
