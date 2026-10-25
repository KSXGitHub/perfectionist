//! A crate spelling visible characters as numeric escapes, which
//! `perfectionist::numeric_escape_for_visible_character` writes out
//! verbatim.

/// Three letters, the first written as a hex escape.
pub const LETTERS: &str = "\x41BC";

/// An arrow, written as a Unicode escape.
pub const ARROW: char = '\u{2192}';

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("numeric_escape_for_visible_character: {LETTERS} {ARROW}\n")
}

#[cfg(test)]
mod tests {
    /// Each literal denotes the same character it did before being
    /// written out, which is the whole of what the rewrite may
    /// change.
    #[test]
    fn the_literals_denote_the_same_characters() {
        assert_eq!(super::LETTERS, "ABC");
        assert_eq!(super::ARROW, '→');
    }
}
