//! [`GetterNamePattern`] — the little pattern language
//! `cloning_getter`'s name roster is written in. One ordered list
//! replaces what used to be a roster of exempt prefixes, a list of
//! prefixes to drop from it, and a boolean for everything the roster
//! did not name: each of those was a separate knob deciding one part
//! of the same question, and a consumer had to know the order the rule
//! consulted them in to predict the answer. Here the order is the one
//! they wrote.
//!
//! Parsing at config-parse time rather than at match time puts the
//! rejection where the message can name the offending entry, and
//! leaves the matcher a `starts_with` against an already-split prefix
//! rather than string surgery repeated for every method in the crate.

use rustc_lexer::{is_id_continue, is_ident};

/// TOML-flavoured type label for [`GetterNamePattern`], surfaced by
/// `tools/gen-docs` in the per-rule field-type column. Sourced here —
/// next to the type definition — so the label is the type's own
/// property rather than a hard-coded match arm in the doc generator.
#[expect(
    dead_code,
    reason = "consumed by `tools/gen-docs` via syntactic scan of this file, not by the runtime"
)]
pub(crate) const TOML_LABEL: &str = "name-pattern string";

/// The prefixes the Rust API guidelines give a fixed meaning, which
/// `cloning_getter` answers for in a clause of its own above the
/// patterns. A pattern may not name one: the clause decides those
/// names whatever the list says, so an entry for one would read as a
/// knob that does nothing.
pub(crate) const CONVERSION_PREFIXES: &[&str] = &["as_", "into_", "to_"];

/// One entry of `cloning_getter`'s `getter_name_patterns` list: a set
/// of method names, and the verdict the entry carries for a name in
/// it.
///
/// Deserialises from a TOML string in one of four forms — `*`,
/// `prefix_*`, `!*`, `!prefix_*` — and rejects anything else at
/// config-parse time.
#[derive(Debug, serde::Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct GetterNamePattern {
    /// Whether the leading `!` was present, which flips the verdict
    /// from "is a getter" to "is not".
    negated: bool,
    /// The names the entry has anything to say about.
    scope: Scope,
}

/// The set of names a pattern covers, with the `_*` or `*` that
/// closed it already stripped.
#[derive(Debug)]
enum Scope {
    /// Written `*`: every name the conversion clause did not already
    /// answer for.
    All,
    /// Written `prefix_*`: every name starting with the stored
    /// prefix, which ends in `_`.
    Prefix(String),
}

impl GetterNamePattern {
    /// Whether this pattern has anything to say about `name`.
    pub(crate) fn matches(&self, name: &str) -> bool {
        match &self.scope {
            Scope::All => true,
            Scope::Prefix(prefix) => name.starts_with(prefix.as_str()),
        }
    }

    /// The verdict this pattern carries for a name it matches: whether
    /// that name is a getter.
    pub(crate) fn selects(&self) -> bool {
        !self.negated
    }
}

impl TryFrom<String> for GetterNamePattern {
    type Error = String;

    fn try_from(pattern: String) -> Result<Self, Self::Error> {
        let (negated, rest) = take_negation(&pattern);
        let scope = take_scope(rest).map_err(|error| error.message(&pattern))?;
        Ok(GetterNamePattern { negated, scope })
    }
}

/// Take the optional leading `!` that flips a pattern's verdict.
/// Always succeeds: a second `!` is left in the remainder, where
/// [`take_scope`] rejects it rather than reading it as a double
/// negative.
fn take_negation(input: &str) -> (bool, &str) {
    match input.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, input),
    }
}

/// Take the whole of a pattern's scope, which is all that may remain
/// once the `!` is off: either the bare `*` that covers every name, or
/// an identifier prefix closed by `_*`.
fn take_scope(input: &str) -> Result<Scope, ScopeError> {
    // The bare `*` first, since an identifier run cannot start with
    // one and the two forms are otherwise disjoint.
    if let Some(((), rest)) = take_wildcard(input)
        && rest.is_empty()
    {
        return Ok(Scope::All);
    }
    let Some((stem, rest)) = take_ident(input) else {
        return Err(ScopeError::Malformed);
    };
    let Some(((), rest)) = take_wildcard(rest) else {
        return Err(ScopeError::MissingWildcard);
    };
    if !rest.is_empty() {
        return Err(ScopeError::TrailingText);
    }
    // The identifier run is greedy over `_`, so a well-formed
    // `clone_*` leaves the underscore on the stem and `prefix*` does
    // not.
    let Some(word) = stem.strip_suffix('_') else {
        return Err(ScopeError::MissingUnderscore);
    };
    if word.is_empty() {
        return Err(ScopeError::EmptyStem);
    }
    if let Some(conversion) = CONVERSION_PREFIXES
        .iter()
        .find(|conversion| stem.starts_with(*conversion))
    {
        return Err(ScopeError::Conversion(conversion));
    }
    Ok(Scope::Prefix(stem.to_owned()))
}

/// Take the `*` that closes every form. Returns the remainder, which
/// the caller requires to be empty — that is what rejects
/// `prefix_*_suffix`, `prefix_**` and `prefix_*_*` alike.
fn take_wildcard(input: &str) -> Option<((), &str)> {
    input.strip_prefix('*').map(|rest| ((), rest))
}

/// Take the longest run of characters a Rust identifier may be
/// spelled with, and require that the run is in fact an identifier.
/// [`is_ident`] is the compiler's own answer to that question, so a
/// prefix this accepts is one a method name could start with and a
/// prefix it rejects is one no method name could.
///
/// Returns `None` when the run is empty, which is what rejects
/// `!!clone_*`, `*_suffix` and the empty string alike, and when the
/// run could not open an identifier, which is what rejects `9th_*`.
fn take_ident(input: &str) -> Option<(&str, &str)> {
    let end = input
        .find(|char: char| !is_id_continue(char))
        .unwrap_or(input.len());
    let (ident, rest) = input.split_at(end);
    is_ident(ident).then_some((ident, rest))
}

/// Why a value is not one of the four forms. Held apart from the text
/// so [`take_scope`] reads as the grammar and each message can quote
/// the whole entry the consumer wrote, not the remainder the parser
/// stopped at.
enum ScopeError {
    /// Nothing that could begin a pattern, including a second `!`.
    Malformed,
    /// A name with no `*` to close it.
    MissingWildcard,
    /// Text after the closing `*`.
    TrailingText,
    /// A prefix with no `_` before the `*`.
    MissingUnderscore,
    /// `_*`: nothing before the `_`.
    EmptyStem,
    /// A prefix the conversion clause already answers for.
    Conversion(&'static str),
}

impl ScopeError {
    /// The message a consumer sees, quoting the entry they wrote.
    fn message(self, pattern: &str) -> String {
        const FORMS: &str = "expected `*`, `prefix_*`, `!*` or `!prefix_*`";
        match self {
            ScopeError::Malformed => {
                format!("{FORMS}, got {pattern:?}")
            }
            ScopeError::MissingWildcard => format!(
                "{FORMS}, got {pattern:?}; a prefix is closed by `*`, so `clone_` is written \
                 `clone_*`",
            ),
            ScopeError::TrailingText => format!(
                "{FORMS}, got {pattern:?}; the `*` ends the pattern, so a name is matched by its \
                 prefix and never by its middle or its end",
            ),
            ScopeError::MissingUnderscore => format!(
                "{FORMS}, got {pattern:?}; a prefix ends in `_`, without which it would match \
                 partway through a word",
            ),
            ScopeError::EmptyStem => format!(
                "{FORMS}, got {pattern:?}; a prefix needs at least one character before the `_`, \
                 or it would match every name that starts with one",
            ),
            ScopeError::Conversion(conversion) => format!(
                "{pattern:?} names the conversion prefix `{conversion}`, which is never a getter \
                 whatever this list says; an entry for it would have no effect",
            ),
        }
    }
}

#[cfg(test)]
mod tests;
