//! What a fixture declares about itself.
//!
//! `fixture.toml` sits at the root of each fixture and names the runs
//! to drive it through. Everything else about a fixture is on disk:
//! the project is the `project/` directory beside that file, a run's
//! configuration is the `dylint.toml` under its directory, and a
//! round's expectations are the `round-<n>.diff` and
//! `round-<n>.lints` files beside it.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One fixture, as `fixture.toml` declares it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// The workspace member whose `main` the runtime-behaviour check
    /// runs. Its stdout is recorded before the first round and
    /// required to be byte-identical after the last one.
    ///
    /// A project where every round is expected to rewrite nothing
    /// may leave it out: a tree the rounds do not touch cannot behave
    /// differently afterwards, so there is nothing for a `main` to
    /// witness. Its own tests are still compiled and run either way.
    pub cli_package: Option<String>,

    /// The runs over this project, driven in the order they are
    /// declared.
    pub runs: Vec<Run>,

    /// The fixture whose build cache this one shares. Defaults to
    /// this fixture itself.
    ///
    /// What a cache has to hold is decided by the dependency list and
    /// by nothing else, so two fixtures declaring the same list want
    /// one cache rather than two, and the second to be warmed gets
    /// the standard library, the registry dependencies and the plugin
    /// from the first. Their members cannot be taken for each other,
    /// cargo keying a unit by package name and the two fixtures
    /// naming theirs apart.
    cache: Option<String>,
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

    /// Whether the last round leaves nothing reported at all.
    /// Defaults to `true`.
    ///
    /// A run sets it to `false` where the project holds shapes the
    /// rules deliberately decline to hand the fixer: those keep being
    /// reported however many rounds are applied, and what the run
    /// asserts about them is that nothing was rewritten. The names
    /// still reported are then held to `round-<n>.lints` like any
    /// other round's, so a decline that turns into a rewrite fails
    /// here.
    #[serde(default = "yes")]
    pub settles: bool,
}

/// What [`Run::settles`] is where the fixture says nothing.
fn yes() -> bool {
    true
}

impl Fixture {
    /// The fixture whose build cache the fixture named `fixture`
    /// uses, which is `fixture` itself unless it says otherwise.
    pub fn cache<'a>(&'a self, fixture: &'a str) -> &'a str {
        self.cache.as_deref().unwrap_or(fixture)
    }

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
