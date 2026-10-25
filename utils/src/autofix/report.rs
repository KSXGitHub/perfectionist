//! Collect what a settling path got wrong, and lay out the evidence.
//!
//! A failure here is about a codebase, not a line, so the message
//! names the fixture and the round and then points at a directory
//! holding the codebase as it stood after every round up to the one
//! that failed. Reading round 2 against round 1 is usually what says
//! which rewrite went wrong, and neither is reconstructible from the
//! stored diffs once a round has diverged from them.
//!
//! The dump outlives the test deliberately: a temp dir that deleted
//! itself as the assertion fired would leave the message pointing at
//! nothing.

use crate::autofix::snapshot::{self, Snapshot};
use core::fmt::Write as _;
use std::path::PathBuf;

/// How to re-record every expectation this harness compares against.
pub const BLESS: &str = "PERFECTIONIST_BLESS_AUTOFIX";

/// What one settling path got wrong, with the codebase of each round
/// it got through kept alongside.
pub struct Report {
    codebase: String,
    path: String,
    failures: Vec<String>,
    rounds: Vec<Snapshot>,
}

impl Report {
    pub fn new(codebase: String, path: String) -> Self {
        Report {
            codebase,
            path,
            failures: Vec::new(),
            rounds: Vec::new(),
        }
    }

    /// Record one thing that went wrong. A path keeps going after
    /// one, so a single run reports every round that diverged rather
    /// than only the first.
    pub fn fail(&mut self, failure: String) {
        self.failures.push(failure);
    }

    /// Record the codebase as it stands. Called once with the
    /// materialised fixture, and once after each round, so the dump
    /// is indexed by round number.
    pub fn record(&mut self, snapshot: Snapshot) {
        self.rounds.push(snapshot);
    }

    pub fn failed(&self) -> bool {
        !self.failures.is_empty()
    }

    /// The failure message, or `None` where the path came out clean.
    /// Dumping is left until here so a clean run writes nothing.
    pub fn finish(self) -> Option<String> {
        if !self.failed() {
            return None;
        }
        let mut message = format!(
            "codebase fixture `{}`, settling path `{}`: {} failure(s)\n",
            self.codebase,
            self.path,
            self.failures.len(),
        );
        for failure in &self.failures {
            writeln!(message, "\n{failure}").expect("write a failure");
        }
        let dump = self.dump();
        writeln!(
            message,
            "\nThe codebase after each round is laid out under\n  {}\n\
             as `round-<n>`, with `round-0` the fixture as stored.\n\
             Re-record every expectation with {BLESS}=1.",
            dump.display(),
        )
        .expect("write the dump location");
        Some(message)
    }

    /// Lay each recorded round out as a full codebase under one
    /// parent directory, and hand back that parent.
    fn dump(&self) -> PathBuf {
        let parent = tempfile::Builder::new()
            .prefix(&format!(
                "perfectionist-autofix-{}-{}-",
                self.codebase, self.path,
            ))
            .tempdir()
            .expect("create a directory to dump the rounds into")
            .keep();
        for (index, round) in self.rounds.iter().enumerate() {
            snapshot::write(&parent.join(format!("round-{index}")), round);
        }
        parent
    }
}
