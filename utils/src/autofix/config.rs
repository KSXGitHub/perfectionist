//! What a codebase fixture declares about itself.
//!
//! `fixture.toml` sits at the root of each codebase fixture and names
//! the settling paths to drive it along. Everything else about a
//! fixture is on disk: the codebase is the `codebase/` directory
//! beside that file, a path's configuration is the `dylint.toml`
//! under its directory, and a round's expectations are the
//! `round-<n>.diff` and `round-<n>.lints` files beside it.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One codebase fixture, as `fixture.toml` declares it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// The workspace member whose `main` the runtime-behaviour check
    /// runs. Its stdout is recorded before the first round and
    /// required to be byte-identical after the last one.
    ///
    /// A codebase where every round is expected to rewrite nothing
    /// may leave it out: a tree the rounds do not touch cannot behave
    /// differently afterwards, so there is nothing for a `main` to
    /// witness. Its own tests are still compiled and run either way.
    pub cli_package: Option<String>,

    /// The settling paths over this codebase, driven in the order
    /// they are declared.
    pub paths: Vec<SettlingPath>,

    /// The codebase whose build cache this one shares. Defaults to
    /// this codebase itself.
    ///
    /// What a cache has to hold is decided by the dependency list and
    /// by nothing else, so two codebases declaring the same list want
    /// one cache rather than two, and the second to be warmed gets
    /// the standard library, the registry dependencies and the plugin
    /// from the first. Their members cannot be taken for each other,
    /// cargo keying a unit by package name and the two codebases
    /// naming theirs apart.
    cache: Option<String>,
}

/// One configuration and the rounds it takes to settle the codebase
/// under it. The configuration itself is the `dylint.toml` under
/// [`SettlingPath::directory`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlingPath {
    /// Names the path's directory under `paths/`, and the path in a
    /// failure report.
    pub name: String,

    /// How many rounds this path takes. The harness applies exactly
    /// this many, so a codebase that would settle in fewer, or need
    /// more, fails rather than being driven to a fixpoint.
    pub rounds: usize,

    /// Whether the last round leaves nothing reported at all.
    /// Defaults to `true`.
    ///
    /// A path sets it to `false` where the codebase holds shapes the
    /// rules deliberately decline to hand the fixer: those keep being
    /// reported however many rounds are applied, and what the path
    /// asserts about them is that nothing was rewritten. The names
    /// still reported are then held to `round-<n>.lints` like any
    /// other round's, so a decline that turns into a rewrite fails
    /// here.
    #[serde(default = "yes")]
    pub settles: bool,
}

/// What [`SettlingPath::settles`] is where the fixture says nothing.
fn yes() -> bool {
    true
}

impl Fixture {
    /// The codebase whose build cache the fixture named `codebase`
    /// uses, which is `codebase` itself unless it says otherwise.
    pub fn cache<'a>(&'a self, codebase: &'a str) -> &'a str {
        self.cache.as_deref().unwrap_or(codebase)
    }

    /// Read the `fixture.toml` at the root of the codebase fixture
    /// `name` holds, under `fixtures_dir`.
    pub fn load(fixtures_dir: &Path, name: &str) -> Self {
        let file = fixtures_dir.join(name).join("fixture.toml");
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
        let fixture: Self = toml::from_str(&text)
            .unwrap_or_else(|error| panic!("parse {}: {error}", file.display()));
        assert!(
            !fixture.paths.is_empty(),
            "{} declares no settling path, so it would assert nothing",
            file.display(),
        );
        fixture
    }
}

impl SettlingPath {
    /// Where this path's configuration and round expectations live,
    /// given the root of the codebase fixture.
    pub fn directory(&self, codebase_dir: &Path) -> PathBuf {
        codebase_dir.join("paths").join(&self.name)
    }
}
