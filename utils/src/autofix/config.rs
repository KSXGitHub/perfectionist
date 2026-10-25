//! What a fixture declares about itself.
//!
//! `fixture.toml` sits at the root of each fixture and names the runs
//! to drive it through. Everything else about a fixture is on disk:
//! the project is the `project/` directory beside that file, a run's
//! configuration is the `dylint.toml` under its directory, and a
//! round's expectations are the `round-<n>.diff` and
//! `round-<n>.stderr` files beside it.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One fixture, as `fixture.toml` declares it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// The workspace member whose `main` the runtime-behaviour check
    /// runs, once the last round has run, against the output the
    /// fixture stores for it.
    pub cli_package: String,

    /// The runs over this project, driven in the order they are
    /// declared.
    pub runs: Vec<Run>,
}

/// One configuration and the rounds it takes to settle the project
/// under it. The configuration itself is the `dylint.toml` under
/// [`Run::directory`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    /// Names the run's directory under `runs/`, and the run in a
    /// failure report.
    pub name: String,

    /// How many rounds this run takes. The harness applies exactly
    /// this many, so a project that would settle in fewer, or need
    /// more, fails rather than being driven to a fixpoint.
    pub rounds: usize,
}

impl Fixture {
    /// Read the `fixture.toml` at the root of the fixture `name`
    /// holds, under `fixtures_dir`.
    pub fn load(fixtures_dir: &Path, name: &str) -> Self {
        let file = fixtures_dir.join(name).join("fixture.toml");
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
        let fixture: Self = toml::from_str(&text)
            .unwrap_or_else(|error| panic!("parse {}: {error}", file.display()));
        assert!(
            !fixture.runs.is_empty(),
            "{} declares no run, so it would assert nothing",
            file.display(),
        );
        fixture
    }
}

impl Run {
    /// Where this run's configuration and round expectations live,
    /// given the root of the fixture.
    pub fn directory(&self, fixture_dir: &Path) -> PathBuf {
        fixture_dir.join("runs").join(&self.name)
    }
}
