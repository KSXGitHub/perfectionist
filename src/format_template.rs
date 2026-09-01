//! Parsing the *contents* of a `format!`-style template string.
//!
//! [`crate::macro_template`] answers "where is the template literal?";
//! this module answers "what is inside it?". A template is a sequence
//! of literal runs, `{{` / `}}` escapes, and `{...}` placeholders, and
//! [`parse_template`] hands back exactly that sequence so a rule can
//! reason about the parts rather than about the raw text.
//!
//! The scanner is written as parser-combinator-style `take_*` functions
//! per the catalogue's parser-style convention (see
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`), each consuming a
//! prefix of its input and returning the remainder.
//!
//! The grammar covered is the one `format!` and `derive_more`'s
//! formatting attributes share: a placeholder is
//! `{` *argument* [`:` *format-spec*] `}`, where the argument is an
//! identifier or an index and the format spec carries no nested braces.
//! Neither part is interpreted here — a caller that cares which trait a
//! spec selects, or which field an argument names, reads the borrowed
//! text itself.

/// One piece of a parsed template.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Segment<'a> {
    /// A run of ordinary text carrying no brace.
    Literal(&'a str),
    /// A `{{` or `}}` escape, carrying the single brace it renders as.
    EscapedBrace(char),
    /// A `{...}` placeholder.
    Placeholder(Placeholder<'a>),
}

/// A `{...}` placeholder, split at its `:` into the argument it
/// interpolates and the format spec applied to it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Placeholder<'a> {
    /// The text between `{` and the `:` (or the closing `}` when there
    /// is none): an identifier, an index, or empty for the implicit
    /// positional form `{}`.
    pub(crate) argument: &'a str,
    /// The text between the `:` and the closing `}`, or `None` when the
    /// placeholder carries no `:` at all. `{x}` yields `None` and `{x:}`
    /// yields `Some("")` — the two are equivalent to `format!`, and a
    /// caller that wants them treated alike uses [`Self::spec`].
    pub(crate) format_spec: Option<&'a str>,
}

impl<'a> Placeholder<'a> {
    /// The format spec with the absent and empty forms collapsed, since
    /// `{x}` and `{x:}` select the same trait and apply the same
    /// (nonexistent) formatting.
    pub(crate) fn spec(&self) -> &'a str {
        self.format_spec.unwrap_or("")
    }
}

/// Split `template` into its segments, or `None` when it is not a
/// well-formed template — an unclosed `{`, or a `}` that neither closes
/// a placeholder nor is doubled. A caller that reaches a template
/// rustc has already accepted still has to handle `None`, because the
/// attribute may not have been compiled as a template at all.
pub(crate) fn parse_template(template: &str) -> Option<Vec<Segment<'_>>> {
    let mut rest = template;
    let mut segments = Vec::new();
    while !rest.is_empty() {
        if let Some((brace, remainder)) = take_escaped_brace(rest) {
            segments.push(Segment::EscapedBrace(brace));
            rest = remainder;
        } else if rest.starts_with('{') {
            let (placeholder, remainder) = take_placeholder(rest)?;
            segments.push(Segment::Placeholder(placeholder));
            rest = remainder;
        } else if rest.starts_with('}') {
            // A lone `}` closes nothing: `take_placeholder` consumes a
            // placeholder's own closing brace, and a literal one has to
            // be written `}}`.
            return None;
        } else {
            let (text, remainder) = take_literal_text(rest);
            segments.push(Segment::Literal(text));
            rest = remainder;
        }
    }
    Some(segments)
}

/// Consume a `{{` or `}}` escape, yielding the single brace it renders
/// as. `None` when the input does not open with a doubled brace.
fn take_escaped_brace(input: &str) -> Option<(char, &str)> {
    let mut chars = input.chars();
    let brace = chars.next()?;
    if brace != '{' && brace != '}' {
        return None;
    }
    (chars.next()? == brace).then(|| (brace, &input[brace.len_utf8() * 2..]))
}

/// Consume the run of ordinary text up to the next brace of either
/// kind. Always succeeds; the run is empty when the input already
/// starts with a brace, which is why [`parse_template`] tries the two
/// brace cases first and never pushes an empty [`Segment::Literal`].
fn take_literal_text(input: &str) -> (&str, &str) {
    let end = input.find(['{', '}']).unwrap_or(input.len());
    input.split_at(end)
}

/// Consume a whole `{...}` placeholder. `None` when the input does not
/// open with `{`, or when no `}` closes it.
///
/// The closing brace is found by scanning forward, which is correct
/// because the format-spec grammar has no nested braces: a width or
/// precision is written `1$` or `.*`, never `{1}`.
fn take_placeholder(input: &str) -> Option<(Placeholder<'_>, &str)> {
    let body = input.strip_prefix('{')?;
    let end = body.find('}')?;
    let (body, rest) = body.split_at(end);
    let (argument, format_spec) = match body.split_once(':') {
        Some((argument, spec)) => (argument, Some(spec)),
        None => (body, None),
    };
    Some((
        Placeholder {
            argument,
            format_spec,
        },
        &rest["}".len()..],
    ))
}

#[cfg(test)]
mod tests;
