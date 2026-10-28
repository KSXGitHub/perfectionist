//! Re-record the `.stderr` fixtures from what the run produced.
//!
//! compiletest has already computed the text a `.stderr` should hold
//! by the time a comparison fails, so re-recording is a copy. What it
//! costs is that the file holding that text is named from
//! `build_base`, which `dylint_testing` leaves at compiletest's
//! default and neither crate's API promises: a drift there leaves a
//! blessing run finding nothing rather than failing, which
//! [`crate::ui_test`] answers by re-raising a failed run that
//! recorded nothing.
//!
//! compiletest never deletes one of those files, so an actual output
//! written by a failure outlives every later run that agrees with its
//! fixture. [`Expectations::clear`] therefore goes first.

use crate::ui_fixtures::fixture_sources;
use derive_more::Display;
use pipe_trait::Pipe;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::{env, fs, io};

pub(crate) const VAR: &str = "PERFECTIONIST_BLESS";

/// compiletest's default stage id, which `dylint_testing` leaves
/// alone and which lands in the name of every file it writes.
const STAGE_ID: &str = "stage-id";

/// Whether this run re-records rather than compares.
///
/// Panicking is the end of the line for a bad value: the `#[test]`
/// that reaches this through [`crate::ui_test`] returns `()`, so
/// there is no caller left to hand a [`Result`] to.
pub(crate) fn enabled() -> bool {
    env::var_os(VAR)
        .as_deref()
        .pipe(parse)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// What a value of [`VAR`] can come out short of.
#[derive(Display)]
#[display("{VAR} must be 'true', 'false', empty, or unset; got: {_0:?}")]
struct BlessValueError(OsString);

/// `true` records, `false` and empty compare, and any other value is
/// rejected rather than read as one of those, so a misspelt opt-in
/// never quietly leaves the run comparing. That tri-state is the one
/// `PERFECTIONIST_CARGO_LOCKED` takes.
fn parse(value: Option<&OsStr>) -> Result<bool, BlessValueError> {
    let Some(value) = value else {
        return Ok(false);
    };
    match value.to_str() {
        Some("true") => Ok(true),
        Some("false" | "") => Ok(false),
        _ => Err(BlessValueError(value.to_owned())),
    }
}

/// A pair of paths per fixture, and nothing read from either: the
/// `.stderr` the tree commits, and the file compiletest writes that
/// fixture's actual output to. Both methods go to the filesystem.
pub(crate) struct Expectations(Vec<Expectation>);

struct Expectation {
    /// The `.stderr` in the repository.
    committed: PathBuf,
    /// Where compiletest leaves its actual output on a mismatch.
    staged: PathBuf,
}

impl Expectations {
    /// The expectations of the committed tree at
    /// `<manifest_dir>/<relative>`, rather than of the copy the run
    /// reads.
    pub(crate) fn of(manifest_dir: &Path, relative: &Path) -> Self {
        Expectations::under(manifest_dir, relative, &env::temp_dir())
    }

    /// With the directory compiletest writes under given rather than
    /// read from the environment, so a test exercises the arithmetic
    /// without writing where a real run looks.
    fn under(manifest_dir: &Path, relative: &Path, staged_root: &Path) -> Self {
        // compiletest names a test after the fixture's path below
        // `src_base`, which is the copy of the first component of
        // `relative`, so that component is what comes off a committed
        // path to reach the name compiletest wrote.
        let root = manifest_dir.join(
            relative
                .components()
                .next()
                .expect("fixture path is not empty"),
        );
        let expectations = fixture_sources(&manifest_dir.join(relative))
            .into_iter()
            .map(|fixture| {
                let below = fixture
                    .strip_prefix(&root)
                    .expect("a fixture sits under its own tree");
                Expectation {
                    committed: fixture.with_extension("stderr"),
                    staged: staged_root
                        .join(below)
                        .with_extension(format!("{STAGE_ID}.stderr")),
                }
            })
            .collect();
        Expectations(expectations)
    }

    /// Delete the file compiletest would write each actual output to,
    /// so that one found on disk after the run was written by that
    /// run.
    pub(crate) fn delete_staged(&self) {
        for expectation in &self.0 {
            if let Err(error) = fs::remove_file(&expectation.staged)
                && error.kind() != io::ErrorKind::NotFound
            {
                panic!("remove {}: {error}", expectation.staged.display());
            }
        }
    }

    /// Copy each actual output compiletest left on disk over the
    /// `.stderr` it belongs to, and hand back the ones replaced. A
    /// fixture whose output matched left no file, so it is skipped.
    pub(crate) fn record(&self) -> Vec<&Path> {
        self.0
            .iter()
            .filter(|expectation| expectation.staged.exists())
            .map(|expectation| {
                fs::copy(&expectation.staged, &expectation.committed).unwrap_or_else(|error| {
                    panic!(
                        "copy {} over {}: {error}",
                        expectation.staged.display(),
                        expectation.committed.display(),
                    )
                });
                expectation.committed.as_path()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
