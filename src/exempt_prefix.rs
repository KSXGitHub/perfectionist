//! [`ExemptPrefix`] — method-name-prefix newtype for `cloning_getter`'s
//! exempt-prefix roster. Encoding the invariants in the type puts the
//! rejection at config-parse time, where the message can name the
//! offending entry, rather than at match time where a malformed prefix
//! would simply never fire.

/// TOML-flavoured type label for [`ExemptPrefix`], surfaced by
/// `tools/gen-docs` in the per-rule field-type column. Sourced here —
/// next to the type definition — so the label is the type's own
/// property rather than a hard-coded match arm in the doc generator.
#[expect(
    dead_code,
    reason = "consumed by `tools/gen-docs` via syntactic scan of this file, not by the runtime"
)]
pub(crate) const TOML_LABEL: &str = "identifier-prefix string";

/// The prefixes `cloning_getter` handles in a clause of its own, above
/// the roster. Listing one here would read as though the roster were
/// what exempts it, and removing it from the roster would then look
/// like a way to make the rule measure it, which no knob can do.
const CONVERSION_PREFIXES: &[&str] = &["as_", "into_", "to_"];

/// A method-name prefix that keeps a method out of `cloning_getter`.
///
/// Deserialises from a TOML string and rejects, at config-parse time,
/// anything that could not serve as one: a value not ending in `_`,
/// which would match partway through a word; a bare `_`, which would
/// match every underscore-led name; and the conversion prefixes, which
/// the rule already exempts unconditionally.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct ExemptPrefix(String);

impl TryFrom<String> for ExemptPrefix {
    type Error = String;

    fn try_from(prefix: String) -> Result<Self, Self::Error> {
        let Some(stem) = prefix.strip_suffix('_') else {
            return Err(format!(
                "expected a prefix ending in `_`, got {prefix:?}; without it the prefix would \
                 match partway through a word",
            ));
        };
        if stem.is_empty() {
            return Err(format!(
                "expected at least one character before the `_`, got {prefix:?}; a bare `_` \
                 would match every name that starts with one",
            ));
        }
        if CONVERSION_PREFIXES.contains(&prefix.as_str()) {
            return Err(format!(
                "`{prefix}` is a conversion prefix, which is never a getter whatever this \
                 roster says; listing it here would have no effect",
            ));
        }
        Ok(ExemptPrefix(prefix))
    }
}

impl From<ExemptPrefix> for String {
    fn from(prefix: ExemptPrefix) -> String {
        prefix.0
    }
}

#[cfg(test)]
mod tests;
