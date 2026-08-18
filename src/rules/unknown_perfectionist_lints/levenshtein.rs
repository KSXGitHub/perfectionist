//! Edit-distance search behind the rule's "did you mean" hint.
//!
//! [`Candidate`] is the entry point: it prepares an unknown lint name
//! once, then measures it against every registered name. Both sides of
//! every comparison are lint names, so [`levenshtein`] works on slices
//! rather than on `&str`, and the element type is whichever unit is
//! cheapest for the candidate at hand.

/// An unknown lint name, prepared for repeated distance measurements
/// against the registered names.
///
/// [`Candidate::distance_to`] runs once per registered lint, so the
/// per-candidate decoding happens here rather than inside that loop —
/// and in the ASCII case there is none to do.
pub(super) enum Candidate<'source> {
    /// An ASCII candidate, compared byte by byte. Every registered lint
    /// name is ASCII too, so a byte comparison yields exactly the
    /// `char`-wise distance while decoding nothing and allocating
    /// nothing: `str::as_bytes` is free, where `Vec<char>` would cost a
    /// heap allocation and four bytes per character.
    Ascii(&'source [u8]),
    /// A candidate carrying non-ASCII characters — Rust identifiers are
    /// not restricted to ASCII — compared character by character.
    ///
    /// Bytes cannot stand in here: they would count a single multi-byte
    /// character as up to four edits and so lose the hint for exactly
    /// the typo that most needs one, a homoglyph. Both
    /// `unicode_ellipsis_in_cоmments` (Cyrillic `о`) and
    /// `unicode_ellipsis_in_cｏmments` (fullwidth `ｏ`) are one character
    /// away from a registered name but two and three bytes away, and
    /// the latter already exceeds the default `suggestion_distance`.
    Unicode(Vec<char>),
}

impl<'source> Candidate<'source> {
    pub(super) fn new(name: &'source str) -> Self {
        if name.is_ascii() {
            Candidate::Ascii(name.as_bytes())
        } else {
            Candidate::Unicode(name.chars().collect())
        }
    }

    /// The edit distance from this candidate to `registered`, which is
    /// one of the plugin's own — hence ASCII — lint names.
    pub(super) fn distance_to(&self, registered: &str) -> usize {
        match self {
            Candidate::Ascii(bytes) => levenshtein(bytes, registered.as_bytes()),
            // Decoding the registered name per comparison is confined
            // to the non-ASCII path, which is reached only while a
            // diagnostic is already being emitted.
            Candidate::Unicode(chars) => {
                let registered: Vec<char> = registered.chars().collect();
                levenshtein(chars, &registered)
            }
        }
    }
}

/// The Levenshtein edit distance between `left` and `right`: the fewest
/// single-element insertions, deletions, and substitutions that turn
/// one sequence into the other.
///
/// Generic over the element type so the caller compares whichever unit
/// is cheapest; see [`Candidate`] for the two this rule uses.
fn levenshtein<Unit: Eq>(left: &[Unit], right: &[Unit]) -> usize {
    let left_len = left.len();
    let right_len = right.len();
    if left_len == 0 {
        return right_len;
    }
    if right_len == 0 {
        return left_len;
    }
    let mut previous_row: Vec<usize> = (0..=right_len).collect();
    let mut current_row: Vec<usize> = vec![0; right_len + 1];
    for i in 1..=left_len {
        current_row[0] = i;
        for j in 1..=right_len {
            let substitution_cost = usize::from(left[i - 1] != right[j - 1]);
            current_row[j] = (previous_row[j] + 1)
                .min(current_row[j - 1] + 1)
                .min(previous_row[j - 1] + substitution_cost);
        }
        core::mem::swap(&mut previous_row, &mut current_row);
    }
    previous_row[right_len]
}

#[cfg(test)]
mod tests;
