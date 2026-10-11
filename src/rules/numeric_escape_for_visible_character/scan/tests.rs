use super::decode_numeric_escape;

/// A numeric escape in the source spelling the scan hands over, and
/// the codepoint it stands for. Raw strings throughout, so the table
/// reads as the bytes a scanned literal holds rather than as this
/// file's own escapes, and codepoints rather than characters, so a
/// row stays readable where no font would render the character.
const DECODED: &[(&str, u32)] = &[
    (r"\x41", 0x41),
    (r"\x7F", 0x7F),
    (r"\x00", 0x0),
    (r"\u{41}", 0x41),
    (r"\u{0}", 0x0),
    (r"\u{2026}", 0x2026),
    (r"\u{1F600}", 0x1F600),
    (r"\u{1_F600}", 0x1F600),
    (r"\u{10FFFF}", 0x10FFFF),
];

/// An escape the rule has nothing to say about, paired with why.
const UNDECODED: &[(&str, &str)] = &[
    (r"\n", "a single-character escape, not a numeric one"),
    (r"\0", "the NUL short-hand, not a numeric escape"),
    (r"\\", "an escaped backslash"),
    (r#"\""#, "an escaped quote"),
    (r"\'", "an escaped apostrophe"),
    (r"\x80", "past U+007F: one byte rather than one character"),
    (r"\xFF", "past U+007F: one byte rather than one character"),
    (r"\xZZ", "not hexadecimal"),
    (r"\u{}", "no digits"),
    (r"\u{D800}", "a surrogate, which is no character"),
    (r"\u{110000}", "past the last codepoint"),
];

#[test]
fn numeric_escapes_decode() {
    for &(escape, codepoint) in DECODED {
        let expected = char::from_u32(codepoint).expect("the table holds no surrogate");
        assert_eq!(
            decode_numeric_escape(escape),
            Some(expected),
            "{escape} should decode to U+{codepoint:04X}",
        );
    }
}

#[test]
fn other_escapes_do_not_decode() {
    for &(escape, reason) in UNDECODED {
        assert_eq!(
            decode_numeric_escape(escape),
            None,
            "{escape} should not decode: {reason}",
        );
    }
}
