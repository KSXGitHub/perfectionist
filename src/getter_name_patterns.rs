//! [`GetterNamePatterns`] — `cloning_getter`'s `getter_name_patterns`
//! list: [`NamePattern`]s under two rules of this rule's own.
//!
//! **The list opens with `*` or `!*`.** Every name it is asked about
//! is then covered by at least its first entry, so the list always has
//! an answer and a reader never has to know a default to work out what
//! one says. `["!*", "get_*"]` reads as "nothing, then `get_*` back",
//! and the alternative it is not — `["get_*"]`, resting on an unstated
//! baseline — cannot be written.
//!
//! **No entry names a conversion prefix.** The rule answers for `as_*`,
//! `to_*` and `into_*` in a clause above the list, so an entry naming
//! one of them, or any longer prefix under one such as `as_ref_*`,
//! covers only names that clause has already decided. Rejecting it says
//! so; accepting it would leave a consumer with a knob that does
//! nothing.
//!
//! Both are the list's rules rather than the language's, which is why
//! they live here and not in [`crate::name_pattern`]: another rule's
//! list has no reason to carry either.

use crate::name_pattern::{NamePattern, verdict};

/// TOML-flavoured type label for [`GetterNamePatterns`], surfaced by
/// `tools/gen-docs` in the per-rule field-type column. Sourced here —
/// next to the type definition — so the label is the type's own
/// property rather than a hard-coded match arm in the doc generator.
///
/// It is spelled as an array because the type deserialises from one;
/// the rules on what may go in it are the field's documentation, not
/// its shape.
#[expect(
    dead_code,
    reason = "consumed by `tools/gen-docs` via syntactic scan of this file, not by the runtime"
)]
pub(crate) const TOML_LABEL: &str = "[name-pattern string]";

/// The prefixes the Rust API guidelines give a fixed meaning, which
/// `cloning_getter` answers for in a clause above this list.
pub(crate) const CONVERSION_PREFIXES: &[&str] = &["as_", "into_", "to_"];

/// `cloning_getter`'s list of name patterns, checked on the way in so
/// that asking it about a name always produces an answer.
#[derive(Debug, serde::Deserialize)]
#[serde(try_from = "Vec<String>")]
pub(crate) struct GetterNamePatterns(Vec<NamePattern>);

impl GetterNamePatterns {
    /// Whether the list says `name` is a getter.
    ///
    /// The first entry covers every name, so [`verdict`] never comes
    /// back undecided here; the fallback is unreachable, and is the
    /// quiet answer rather than the loud one so that a future way to
    /// build a list without the check fails safe.
    pub(crate) fn says_getter(&self, name: &str) -> bool {
        verdict(&self.0, name).unwrap_or(false)
    }
}

impl TryFrom<Vec<String>> for GetterNamePatterns {
    type Error = String;

    fn try_from(entries: Vec<String>) -> Result<Self, Self::Error> {
        let patterns = entries
            .into_iter()
            .map(parse_entry)
            .collect::<Result<Vec<NamePattern>, Self::Error>>()?;
        let Some(first) = patterns.first() else {
            return Err(
                "expected at least one pattern, got an empty list; write `[\"!*\"]` for a rule \
                 that measures nothing beyond the names it settles itself, or `[\"*\"]` for one \
                 that measures every name"
                    .to_owned(),
            );
        };
        // Only `*` and `!*` cover every name, and `prefix` is `None`
        // for exactly those two.
        if first.prefix().is_some() {
            return Err(
                "expected the first pattern to be `\"*\"` or `\"!*\"`; it says what every name \
                 means before the rest of the list narrows it, so a list that opens with a \
                 prefix would leave the names it does not mention resting on a default the \
                 reader has to know"
                    .to_owned(),
            );
        }
        Ok(GetterNamePatterns(patterns))
    }
}

/// Parse one entry and reject it where it names a conversion prefix.
fn parse_entry(entry: String) -> Result<NamePattern, String> {
    // The grammar first, so an entry that is both ill-formed and
    // conversion-shaped is reported for its shape.
    let pattern = NamePattern::try_from(entry.clone())?;
    if let Some(prefix) = pattern.prefix()
        && let Some(conversion) = CONVERSION_PREFIXES
            .iter()
            .find(|conversion| prefix.starts_with(*conversion))
    {
        return Err(format!(
            "{entry:?} names the conversion prefix `{conversion}`, which is never a getter \
             whatever this list says; an entry for it would have no effect",
        ));
    }
    Ok(pattern)
}

#[cfg(test)]
mod tests;
