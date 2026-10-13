//! Read the version out of what `cargo dylint --version` prints.
//!
//! The output is clap's version banner: the program's own name, then
//! whitespace, then the version. Reading it by that grammar is what
//! keeps output that is not a banner from reaching the comparison as
//! a version, which taking the output's last word does not.
//!
//! The scanner is written as parser-combinator-style `take_*`
//! functions per the catalogue's parser-style convention (see
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`), each consuming a
//! prefix of its input and returning the remainder.

/// The name clap prints ahead of the version, which it takes from the
/// binary's own name.
const PROGRAM_NAME: &str = "cargo-dylint";

/// Take the version out of a whole banner. Whatever follows the
/// version is left unread, so a banner that grows a suffix still
/// yields its version.
pub(super) fn take_version(input: &str) -> Option<&str> {
    let ((), input) = take_whitespace(input);
    let ((), input) = take_program_name(input)?;
    let ((), input) = take_separator(input)?;
    let (version, _) = take_token(input)?;
    Some(version)
}

/// Take a run of whitespace, which may be empty. Always succeeds: the
/// banner carries no padding of its own, so a run is there only when
/// the output arrived with one.
fn take_whitespace(input: &str) -> ((), &str) {
    ((), input.trim_start())
}

/// Take the program's name. Returns `None` for output that opens with
/// anything else, which is what keeps prose from reading as a banner
/// however much of it looks like one.
fn take_program_name(input: &str) -> Option<((), &str)> {
    input.strip_prefix(PROGRAM_NAME).map(|rest| ((), rest))
}

/// Take the whitespace that separates the name from the version, of
/// which there has to be some. Returns `None` for an empty run, which
/// is what rejects a version run onto the end of the name.
fn take_separator(input: &str) -> Option<((), &str)> {
    let ((), rest) = take_whitespace(input);
    (rest.len() < input.len()).then_some(((), rest))
}

/// Take the longest run of non-whitespace, which is how a version
/// reaches the output. Returns `None` for an empty run, which is what
/// rejects a banner that stops after the name.
fn take_token(input: &str) -> Option<(&str, &str)> {
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    let (token, rest) = input.split_at(end);
    (!token.is_empty()).then_some((token, rest))
}

#[cfg(test)]
mod tests;
