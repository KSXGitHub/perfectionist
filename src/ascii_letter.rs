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
/// character (`unicode_ellipsis_*::extra_flagged_chars`); `AsciiLetter`'s
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
/// `extra_denied_idents` knobs so the invariant the rule
/// documentation advertises ("each entry is a single ASCII letter,
/// `a`-`z`, `A`-`Z`") is part of the type rather than carried by a
/// `#[serde(deserialize_with = "...")]` attribute by convention.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(try_from = "char")]
pub(crate) struct AsciiLetter(char);

impl TryFrom<char> for AsciiLetter {
    type Error = String;

    fn try_from(char: char) -> Result<Self, Self::Error> {
        char.is_ascii_alphabetic()
            .then_some(AsciiLetter(char))
            .ok_or_else(|| format!("expected a single ASCII letter (a-z, A-Z), got {char:?}"))
    }
}

impl From<AsciiLetter> for char {
    fn from(letter: AsciiLetter) -> char {
        letter.0
    }
}

#[cfg(test)]
mod tests;
