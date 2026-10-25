//! Collect the lint names a check of the fixture reported.
//!
//! The harness asks cargo for `--message-format=json` rather than
//! reading the rendered diagnostics, because a rendered diagnostic
//! quotes the source line it fired on, and a fixture's own source can
//! hold the text of a lint name. The JSON carries the name in a field
//! of its own, where nothing the fixture says can reach it.
//!
//! Only a diagnostic carrying a code is collected, which is every
//! lint and no cargo or rustc summary ("2 warnings emitted", "For
//! more information about this error, try ..."). Whether the check
//! itself succeeded is a separate question, and the caller asks it of
//! the exit status.

use serde::Deserialize;
use std::collections::BTreeSet;

/// One line of `--message-format=json`, pared down to the two fields
/// the harness reads. Cargo emits several other `reason`s, and a
/// `compiler-message` from a dependency rather than a workspace
/// member, both of which fall out as a `message` that holds no code.
#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    message: Option<Diagnostic>,
}

#[derive(Deserialize)]
struct Diagnostic {
    code: Option<DiagnosticCode>,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    code: String,
}

/// The lint names `stdout` reported, deduplicated and in ascending
/// order, so the stored expectation reads as a set rather than as the
/// order cargo happened to compile the members in.
pub fn reported(stdout: &str) -> BTreeSet<String> {
    stdout
        .lines()
        .map(serde_json::from_str::<CargoMessage>)
        .filter_map(Result::ok)
        .filter(|message| message.reason == "compiler-message")
        .filter_map(|message| message.message?.code)
        .map(|code| code.code)
        .collect()
}

/// The stored form of a round's remaining lint names: one per line,
/// ascending. An empty set renders as the empty string, which is what
/// a settled codebase stores.
pub fn render(names: &BTreeSet<String>) -> String {
    names
        .iter()
        .map(|name| format!("{name}\n"))
        .collect::<Vec<_>>()
        .concat()
}

#[cfg(test)]
mod tests;
