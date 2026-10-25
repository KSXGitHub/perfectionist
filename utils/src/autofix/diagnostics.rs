//! Collect the diagnostics a check reported, and normalise them into
//! the text a round stores.
//!
//! The harness asks cargo for `--message-format=json` rather than
//! reading the rendered stream off stderr, for two reasons. Cargo
//! renders diagnostics in whatever order its jobs finish, so the
//! stream is not reproducible, while the JSON carries one object per
//! diagnostic for the harness to put in an order of its own. And the
//! JSON names each diagnostic's lint in a field, where nothing a
//! project's own source says can reach it, which is what the settling
//! check reads.
//!
//! Normalisation follows the rules
//! [`crate::ui_fixtures`] states for a `.stderr` under `ui/`, and for
//! the same reason: a span header's `line:column` would otherwise
//! renumber every header below an inserted line. The absolute paths
//! this harness works in are normalised too, since a temp directory's
//! name is different on every run.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

mod normalise;

/// Everything a check reported, kept in the two forms the harness
/// needs: the lint names for the settling check, and the rendered
/// diagnostics for the stored expectation.
#[derive(Default)]
pub struct Reported {
    /// Every lint reported, deduplicated and in ascending order.
    ///
    /// Only a diagnostic carrying a code is counted, which is every
    /// lint and no cargo or rustc summary ("2 warnings emitted", "For
    /// more information about this error, try ...").
    pub lints: BTreeSet<String>,

    /// Each diagnostic as rustc rendered it, normalised and
    /// deduplicated.
    ///
    /// A check covers every target, so a finding in a file the lib
    /// and the lib-test both compile is reported once per target,
    /// which cargo's own tally calls a duplicate. What a stored
    /// expectation is for is the set of shapes the rules still find,
    /// and the targets that happened to see each one say nothing
    /// about that.
    rendered: BTreeSet<String>,
}

/// One line of `--message-format=json`, pared down to the fields the
/// harness reads. Cargo emits several other `reason`s, and a
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
    rendered: Option<String>,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    code: String,
}

impl Reported {
    /// Add everything `stdout` reported. `project_dir` is the
    /// directory the check ran in, whose name a diagnostic can carry
    /// and which differs on every run.
    pub fn extend(&mut self, stdout: &str, project_dir: &Path) {
        let diagnostics = stdout
            .lines()
            .map(serde_json::from_str::<CargoMessage>)
            .filter_map(Result::ok)
            .filter(|message| message.reason == "compiler-message")
            .filter_map(|message| message.message);
        for diagnostic in diagnostics {
            if let Some(code) = diagnostic.code {
                self.lints.insert(code.code);
            }
            if let Some(rendered) = diagnostic.rendered {
                self.rendered.insert(normalise::diagnostic(
                    &rendered,
                    &project_dir.display().to_string(),
                ));
            }
        }
    }

    /// The stored form: every rendered diagnostic, in ascending order
    /// so the file does not move with the order cargo happened to
    /// compile the members in. A check that reported nothing renders
    /// as the empty string, which is what a settled project stores.
    pub fn render(&self) -> String {
        self.rendered
            .iter()
            .map(|block| format!("{block}\n"))
            .collect::<Vec<_>>()
            .concat()
    }
}

#[cfg(test)]
mod tests;
