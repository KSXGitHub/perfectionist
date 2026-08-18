//! Levenshtein edit distance over ASCII lint names.
//!
//! Both sides of every comparison are lint names, and every lint name
//! this plugin registers is ASCII, so the distance is measured over
//! `[u8]`: `str::as_bytes` decodes nothing and allocates nothing, where
//! a `Vec<char>` would cost a heap allocation per comparison and four
//! bytes per character. A candidate that is not ASCII cannot be a
//! near-miss of an ASCII name and never reaches here — the rule names
//! the offending character instead.

/// The Levenshtein edit distance between `left` and `right`: the fewest
/// single-byte insertions, deletions, and substitutions that turn one
/// into the other.
///
/// Both sides are ASCII, where one byte is one character, so this is
/// the character-wise distance too.
pub(super) fn levenshtein(left: &[u8], right: &[u8]) -> usize {
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
