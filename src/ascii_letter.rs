//! `AsciiLetter` — single-ASCII-letter newtype shared between the
//! `single_letter_*` rules' allow-list configuration knobs. Encoding
//! the "ASCII alphabetic" invariant in the type system retires the
//! convention-only `#[serde(deserialize_with = "deserialize_ascii_letters")]`
//! attribute the rules used to carry on their `Vec<char>` fields.

/// TOML-flavoured type label for [`AsciiLetter`], surfaced by
/// `tools/gen-docs` in the per-rule field-type column. Sourced here
/// — next to the type definition — so the label is the type's own
/// property rather than a hard-coded match arm in the doc generator.
///
/// `char` keeps the broader `single-character string` label for the
/// codepoint-shaped fields that genuinely accept any Unicode
/// character (`unicode_ellipsis_*::also_flag`); `AsciiLetter`'s
/// label is strictly narrower because every value satisfies
/// `char::is_ascii_alphabetic`.
#[expect(
    dead_code,
    reason = "consumed by `tools/gen-docs` via syntactic scan of this file, not by the runtime"
)]
pub(crate) const TOML_LABEL: &str = "single-letter string";

/// A single ASCII letter (`a`..=`z` or `A`..=`Z`).
///
/// Deserialises from a TOML single-character string and rejects any
/// non-alphabetic codepoint with a clear error message at
/// config-parse time. The `single_letter_*` rules use
/// `Vec<AsciiLetter>` for their `extra_allowed_idents` /
/// `ignore_allowed_idents` knobs so the invariant the rule
/// documentation advertises ("each entry is a single ASCII letter,
/// `a`-`z`, `A`-`Z`") is part of the type rather than carried by a
/// `#[serde(deserialize_with = "...")]` attribute by convention.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(try_from = "char")]
pub(crate) struct AsciiLetter(char);

impl TryFrom<char> for AsciiLetter {
    type Error = String;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        if value.is_ascii_alphabetic() {
            Ok(Self(value))
        } else {
            // Embed the offending codepoint so a TOML author with a
            // mixed-valid/invalid list (`["a", "1", "c"]`) can see
            // *which* entry the validator rejected. Serde-toml's
            // wrapping error reports the source span, but not the
            // value, so without this the user only learns that
            // "some" entry failed.
            Err(format!(
                "expected a single ASCII letter (a-z, A-Z), got {value:?}",
            ))
        }
    }
}

impl From<AsciiLetter> for char {
    fn from(letter: AsciiLetter) -> char {
        letter.0
    }
}

#[cfg(test)]
mod tests {
    use super::AsciiLetter;

    /// Driver that pipes a TOML array through `AsciiLetter`'s serde
    /// impl via a synthetic struct, mirroring how the real rule
    /// `Config`s use it. Returns the deserialised letters on success
    /// or the serde error message on failure, so the tests can pin
    /// both the happy and the rejection paths.
    fn parse(toml_text: &str) -> Result<Vec<char>, String> {
        #[derive(serde::Deserialize)]
        struct Wrap {
            letters: Vec<AsciiLetter>,
        }
        toml::from_str::<Wrap>(toml_text)
            .map(|wrap| wrap.letters.into_iter().map(char::from).collect())
            .map_err(|err| err.to_string())
    }

    #[test]
    fn empty_list_is_accepted() {
        assert_eq!(parse("letters = []").unwrap(), Vec::<char>::new());
    }

    #[test]
    fn ascii_letters_are_accepted() {
        assert_eq!(parse(r#"letters = ["x", "Y"]"#).unwrap(), vec!['x', 'Y']);
    }

    #[test]
    fn multi_character_string_is_rejected_at_parse_time() {
        // serde-toml rejects a multi-codepoint string before our
        // `TryFrom<char>` gets a chance to run; the error message
        // doesn't matter as long as the TOML fails to parse.
        assert!(parse(r#"letters = ["xy"]"#).is_err());
    }

    #[test]
    fn ascii_digit_is_rejected_with_our_message() {
        let error = parse(r#"letters = ["1"]"#).unwrap_err();
        assert!(
            error.contains("expected a single ASCII letter"),
            "unexpected error message: {error}",
        );
        // The offending codepoint must round-trip into the rendered
        // error so a user with a mixed-valid/invalid array can find
        // which entry failed without counting indices.
        assert!(
            error.contains("'1'"),
            "error should name the offending character: {error}",
        );
    }

    #[test]
    fn non_ascii_letter_is_rejected_with_our_message() {
        let error = parse(r#"letters = ["é"]"#).unwrap_err();
        assert!(
            error.contains("expected a single ASCII letter"),
            "unexpected error message: {error}",
        );
        assert!(
            error.contains("'é'") || error.contains(r"'\u"),
            "error should name the offending character: {error}",
        );
    }
}
