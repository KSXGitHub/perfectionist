//! Collect what a run got wrong, and lay out the evidence.
//!
//! A failure here is about a project, not a line, so the message
//! names the fixture and the round and then points at a directory
//! holding the project as it stood after every round up to the one
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

/// What one run got wrong, with the project of each round it got
/// through kept alongside.
pub struct Report {
    fixture: String,
    run: String,
    failures: Vec<String>,
    rounds: Vec<Snapshot>,
}

impl Report {
    pub fn new(fixture: String, run: String) -> Self {
        Report {
            fixture,
            run,
            failures: Vec::new(),
            rounds: Vec::new(),
        }
    }

    /// Record one thing that went wrong. A run keeps going after one,
    /// so a single pass reports every round that diverged rather than
    /// only the first.
    pub fn fail(&mut self, failure: String) {
        self.failures.push(failure);
    }

    /// Record the project as it stands. Called once with the
    /// materialised project, and once after each round, so the dump
    /// is indexed by round number.
    pub fn record(&mut self, snapshot: Snapshot) {
        self.rounds.push(snapshot);
    }

    pub fn failed(&self) -> bool {
        !self.failures.is_empty()
    }

    /// The failure message, or `None` where the run came out clean.
    /// Dumping is left until here so a clean run writes nothing.
    pub fn finish(self) -> Option<String> {
        if !self.failed() {
            return None;
        }
        let mut message = format!(
            "fixture `{}`, run `{}`: {} failure(s)\n",
            self.fixture,
            self.run,
            self.failures.len(),
        );
        for failure in &self.failures {
            writeln!(message, "\n{failure}").expect("write a failure");
        }
        let dump = self.dump();
        writeln!(
            message,
            "\nThe project after each round is laid out under\n  {}\n\
             as `round-<n>`, with `round-0` the project as stored.\n\
             Re-record every expectation with {BLESS}=1.",
            dump.display(),
        )
        .expect("write the dump location");
        Some(message)
    }

    /// Lay each recorded round out as a full project under one parent
    /// directory, and hand back that parent.
    fn dump(&self) -> PathBuf {
        let parent = tempfile::Builder::new()
            .prefix(&format!(
                "perfectionist-autofix-{}-{}-",
                self.fixture, self.run,
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
