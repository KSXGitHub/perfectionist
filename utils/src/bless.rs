//! Re-record what the UI fixtures are held to, from what the run
//! itself produced.
//!
//! compiletest already computes the text a `.stderr` should hold. On
//! a mismatch it writes its normalised actual output to a file of its
//! own and prints where, so re-recording is a copy rather than a
//! rendering, and the work here is naming that file and knowing when
//! it is this run's.
//!
//! Naming it means knowing `build_base`, the directory compiletest
//! writes under. `dylint_testing` builds compiletest's `Config`
//! itself and leaves that field at its default, the system temp
//! directory, so a fixture's actual output lands at
//! `<temp dir>/<the fixture's path below `src_base`>` with its `.rs`
//! replaced by [`STAGE_ID`] and `stderr`. Neither crate's API
//! promises that, and a drift would leave [`Expectations::harvest`]
//! finding nothing at all rather than failing, which is why it
//! reports what it replaced and [`crate::ui_test`] re-raises a failed
//! run that replaced none.
//!
//! Knowing it is this run's takes a deletion first. compiletest never
//! removes one of those files, so an actual written by a failure
//! outlives every later run that agrees with its fixture, and a
//! harvest on its own would copy that stale text over a `.stderr` the
//! current run was happy with. [`Expectations::clear`] goes first and
//! makes a file found afterwards mean the run in between wrote it.

use crate::ui_fixtures::fixtures_with_stderr;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// Set this to `true` to re-record the `.stderr` fixtures.
pub(crate) const VAR: &str = "PERFECTIONIST_BLESS";

/// compiletest's own default stage id, which `dylint_testing` leaves
/// alone and which it puts in the name of every file it writes.
const STAGE_ID: &str = "stage-id";

/// Whether this run re-records the `.stderr` fixtures rather than
/// holding them to what they contain.
///
/// `true` turns re-recording on. `false`, empty and unset leave it
/// off, and any other value is rejected rather than read as one of
/// those, so a misspelt opt-in never quietly fails to apply. That
/// tri-state is the one `PERFECTIONIST_CARGO_LOCKED` and the
/// `commit-msg` hook's long-subject opt-out already take.
pub(crate) fn enabled() -> bool {
    parse(env::var_os(VAR).as_deref())
}

/// What [`enabled`] makes of one value, split out so that the
/// tri-state can be exercised without setting a variable the rest of
/// the process shares.
fn parse(value: Option<&OsStr>) -> bool {
    let Some(value) = value else {
        return false;
    };
    match value.to_str() {
        Some("true") => true,
        Some("false" | "") => false,
        _ => panic!("{VAR} must be 'true', 'false', empty, or unset; got: {value:?}"),
    }
}

/// The `.stderr` files one fixture tree commits, each paired with the
/// file compiletest writes that fixture's actual output to.
pub(crate) struct Expectations(Vec<Expectation>);

/// One committed `.stderr` and the actual output that would replace
/// it.
struct Expectation {
    /// The `.stderr` in the repository.
    committed: PathBuf,
    /// Where compiletest leaves its actual output on a mismatch.
    staged: PathBuf,
}

impl Expectations {
    /// The expectations of the fixture tree at
    /// `<manifest_dir>/<relative>`, which is the committed tree and
    /// not the copy the run reads.
    ///
    /// Only a `.rs` that already has a `.stderr` is included, which
    /// is the set [`crate::ui_fixtures`] injects its normalising
    /// directives into. A fixture without one is linted unnormalised,
    /// so its actual output carries the real line and column numbers
    /// a committed `.stderr` must not: re-recording from it would
    /// write a fixture that the next edit above any diagnostic
    /// breaks. Give a new fixture an empty `.stderr` first and the
    /// two sets agree again.
    pub(crate) fn of(manifest_dir: &Path, relative: &Path) -> Self {
        Expectations::under(manifest_dir, relative, &env::temp_dir())
    }

    /// The expectations of that tree, with the directory compiletest
    /// writes under given rather than read from the environment, so a
    /// test can exercise the arithmetic without writing where a real
    /// run would look.
    fn under(manifest_dir: &Path, relative: &Path, staged_root: &Path) -> Self {
        // compiletest names a test after the fixture's path below
        // `src_base`, and `src_base` is the copy of the first
        // component of `relative`, so that same first component is
        // what comes off a committed path to reach the name
        // compiletest wrote.
        let root = manifest_dir.join(
            relative
                .components()
                .next()
                .expect("fixture path is not empty"),
        );
        let expectations = fixtures_with_stderr(&manifest_dir.join(relative))
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

    /// Delete every staged actual, so that one found after the run was
    /// written by that run.
    pub(crate) fn clear(&self) {
        for expectation in &self.0 {
            match fs::remove_file(&expectation.staged) {
                Ok(()) => (),
                // Nothing staged is the ordinary case: compiletest
                // writes one only where a comparison failed, so most
                // fixtures have none to begin with.
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => panic!("remove {}: {error}", expectation.staged.display()),
            }
        }
    }

    /// Copy each staged actual over the `.stderr` it belongs to, and
    /// hand back the ones replaced.
    pub(crate) fn harvest(&self) -> Vec<&Path> {
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
