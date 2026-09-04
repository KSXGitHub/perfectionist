//! The permutation algorithm used by `unordered_derives`.
//!
//! [`desired_order`] returns a permutation of `0..entries.len()`
//! describing the target source order under a chosen [`Style`]. The
//! identity permutation means the entries are already in order.

use crate::derive_list::DeriveEntry;
use core::cmp::Ordering;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Style {
    /// Every trait name must appear in ASCII-case-insensitive
    /// alphabetical order.
    #[default]
    Alphabetical,
    /// Traits listed in the configured `prefix` come first, in the
    /// listed order; remaining traits are sorted alphabetically
    /// after.
    PrefixThenAlphabetical,
}

/// Return a permutation of `0..entries.len()` describing the desired
/// source order under `style`. A return value equal to the identity
/// permutation means the entries are already in order.
pub(super) fn desired_order(
    style: Style,
    prefix: &[String],
    entries: &[DeriveEntry],
) -> Vec<usize> {
    match style {
        Style::Alphabetical => {
            let mut indices: Vec<usize> = (0..entries.len()).collect();
            // `slice::sort_by` is stable, so equal-key entries
            // retain their original relative order.
            indices.sort_by(|left, right| {
                ascii_ci_cmp(entries[*left].name.as_str(), entries[*right].name.as_str())
            });
            indices
        }
        Style::PrefixThenAlphabetical => {
            let mut used = vec![false; entries.len()];
            let mut order: Vec<usize> = Vec::with_capacity(entries.len());
            // Walk the configured prefix in order; for each
            // prefix name, find the first not-yet-used entry
            // that matches and append it. An entry that appears
            // in the prefix list but not in the user's derive
            // is silently skipped — the prefix is "preferred
            // ordering", not "required to be present".
            for prefix_name in prefix {
                for (index, entry) in entries.iter().enumerate() {
                    if !used[index] && entry.name.as_str() == prefix_name.as_str() {
                        order.push(index);
                        used[index] = true;
                        break;
                    }
                }
            }
            let mut rest: Vec<usize> = (0..entries.len()).filter(|index| !used[*index]).collect();
            rest.sort_by(|left, right| {
                ascii_ci_cmp(entries[*left].name.as_str(), entries[*right].name.as_str())
            });
            order.extend(rest);
            order
        }
    }
}

pub(super) fn is_identity(permutation: &[usize]) -> bool {
    permutation
        .iter()
        .enumerate()
        .all(|(position, &index)| position == index)
}

/// `Ordering::Less` if `left` precedes `right` in
/// ASCII-case-insensitive alphabetical order. Lowercases byte-by-byte
/// during the comparison rather than allocating a normalised form.
fn ascii_ci_cmp(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}
