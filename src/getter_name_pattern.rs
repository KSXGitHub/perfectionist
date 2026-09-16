//! [`GetterNamePattern`] — `cloning_getter`'s name patterns: a
//! [`NamePattern`] that may not name a conversion prefix.
//!
//! The rule answers for `as_*`, `to_*` and `into_*` in a clause above
//! its pattern list, so an entry naming one of them — or any longer
//! prefix under one, such as `as_ref_*` — covers only names that
//! clause has already decided, and could not change an outcome
//! whatever verdict it carried. Rejecting it at config-parse time says
//! so; accepting it would leave a consumer with a knob that silently
//! does nothing.
//!
//! The ban is this rule's and not the language's, which is why it
//! lives in a newtype here rather than in [`crate::name_pattern`]: a
//! list belonging to some other rule has no reason to carry it.

use crate::name_pattern::NamePattern;

/// TOML-flavoured type label for [`GetterNamePattern`], surfaced by
/// `tools/gen-docs` in the per-rule field-type column. Sourced here —
/// next to the type definition — so the label is the type's own
/// property rather than a hard-coded match arm in the doc generator.
///
/// It matches the label on the type this one wraps: the ban narrows
/// which patterns are accepted, not how one is written, and the label
/// describes the shape a consumer types.
#[expect(
    dead_code,
    reason = "consumed by `tools/gen-docs` via syntactic scan of this file, not by the runtime"
)]
pub(crate) const TOML_LABEL: &str = "name-pattern string";

/// The prefixes the Rust API guidelines give a fixed meaning, which
/// `cloning_getter` answers for in a clause above its pattern list.
pub(crate) const CONVERSION_PREFIXES: &[&str] = &["as_", "into_", "to_"];

/// One entry of `cloning_getter`'s `getter_name_patterns` list.
///
/// Deserialises from a TOML string in the same four forms
/// [`NamePattern`] accepts — `*`, `prefix_*`, `!*`, `!prefix_*` — and
/// additionally rejects any entry whose prefix falls under a
/// conversion prefix.
#[derive(Debug, serde::Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct GetterNamePattern(NamePattern);

impl GetterNamePattern {
    /// Whether this pattern has anything to say about `name`.
    pub(crate) fn matches(&self, name: &str) -> bool {
        self.0.matches(name)
    }

    /// The verdict this pattern carries for a name it matches: whether
    /// that name is a getter.
    pub(crate) fn selects(&self) -> bool {
        self.0.selects()
    }
}

impl TryFrom<String> for GetterNamePattern {
    type Error = String;

    fn try_from(pattern: String) -> Result<Self, Self::Error> {
        // The grammar first, so a malformed entry is reported as
        // malformed rather than as one that happens not to name a
        // conversion.
        let parsed = NamePattern::try_from(pattern.clone())?;
        if let Some(prefix) = parsed.prefix()
            && let Some(conversion) = CONVERSION_PREFIXES
                .iter()
                .find(|conversion| prefix.starts_with(*conversion))
        {
            return Err(format!(
                "{pattern:?} names the conversion prefix `{conversion}`, which is never a getter \
                 whatever this list says; an entry for it would have no effect",
            ));
        }
        Ok(GetterNamePattern(parsed))
    }
}

#[cfg(test)]
mod tests;
