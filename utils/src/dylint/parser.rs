//! Read the version out of what `cargo dylint --version` prints.
//!
//! The output is clap's version banner: the program's own name,
//! whitespace, the version. Reading it by that grammar is what keeps
//! output that is not a banner from reaching the comparison as a
//! version.
//!
//! The scanner is written as parser-combinator-style `take_*`
//! functions per the catalogue's parser-style convention (see
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`), each consuming a
//! prefix of its input and returning the remainder.

/// The name clap prints ahead of the version, which it takes from the
/// binary's own name.
const PROGRAM_NAME: &str = "cargo-dylint";

/// Take the version out of a whole banner. Returns `None` where
/// anything but whitespace follows it, which is what keeps output
/// this grammar does not cover from passing as a version.
pub(super) fn take_version(input: &str) -> Option<&str> {
    let ((), input) = take_whitespace(input);
    let ((), input) = take_program_name(input)?;
    let ((), input) = take_separator(input)?;
    let (version, input) = take_token(input)?;
    let ((), input) = take_whitespace(input);
    input.is_empty().then_some(version)
}

/// Take a run of whitespace, which may be empty.
fn take_whitespace(input: &str) -> ((), &str) {
    ((), input.trim_start())
}

/// Take the program's name. Returns `None` for output that opens with
/// anything else, which is what keeps prose from reading as a banner.
fn take_program_name(input: &str) -> Option<((), &str)> {
    input.strip_prefix(PROGRAM_NAME).map(|rest| ((), rest))
}

/// Take the whitespace between the name and the version. Returns
/// `None` for an empty run, which is what rejects a version run onto
/// the end of the name.
fn take_separator(input: &str) -> Option<((), &str)> {
    let ((), rest) = take_whitespace(input);
    (rest.len() < input.len()).then_some(((), rest))
}

/// Take the longest run of non-whitespace. Returns `None` for an
/// empty run, which is what rejects a banner that stops after the
/// name.
fn take_token(input: &str) -> Option<(&str, &str)> {
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    let (token, rest) = input.split_at(end);
    (!token.is_empty()).then_some((token, rest))
}

#[cfg(test)]
mod tests;
