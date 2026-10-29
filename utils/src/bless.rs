//! Re-record the `.stderr` fixtures from what the run produced.
//!
//! Where compiletest writes a fixture's actual output is promised by
//! no API: the directory is [`crate::build_base`]'s, and the file name
//! is built from compiletest's own defaults, which `dylint_testing`
//! leaves alone.
//!
//! compiletest never deletes one of those files, so a stale one has to
//! go before the run rather than after.

use crate::build_base;
use crate::ui_fixtures::fixture_sources;
use derive_more::Display;
use pipe_trait::Pipe;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{env, fs, io};

const BLESS_ENV: &str = "PERFECTIONIST_BLESS";

/// compiletest's default.
const STAGE_ID: &str = "stage-id";

/// Whether this run re-records rather than compares.
pub(crate) static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    BLESS_ENV
        .pipe(env::var_os)
        .unwrap_or_default()
        .pipe_deref(parse)
        .unwrap_or_else(|error| panic!("{error}"))
});

#[derive(Display)]
#[display("{BLESS_ENV} must be 'true', 'false', empty, or unset; got: {_0:?}")]
struct BlessValueError<'a>(&'a OsStr);

fn parse(value: &OsStr) -> Result<bool, BlessValueError<'_>> {
    match value.as_encoded_bytes() {
        b"true" => Ok(true),
        b"false" | b"" => Ok(false),
        _ => Err(BlessValueError(value)),
    }
}

pub(crate) struct Expectations(Vec<Expectation>);

struct Expectation {
    /// The `.stderr` the repository commits.
    committed: PathBuf,
    /// Where compiletest writes the actual output on a mismatch.
    staged: PathBuf,
}

impl Expectations {
    /// The expectations for the fixtures under
    /// `<manifest_dir>/<relative>`.
    pub(crate) fn of(manifest_dir: &Path, relative: &Path) -> Self {
        Expectations::under(manifest_dir, relative, &build_base::path())
    }

    /// [`Expectations::of`] with `staged_root` given rather than read
    /// from the environment.
    fn under(manifest_dir: &Path, relative: &Path, staged_root: &Path) -> Self {
        // compiletest names a test by the fixture's path below
        // `src_base`, which is a copy of `relative`'s first component.
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

    /// Delete the file compiletest writes each actual output to.
    pub(crate) fn delete_staged(&self) {
        for expectation in &self.0 {
            if let Err(error) = fs::remove_file(&expectation.staged)
                && error.kind() != io::ErrorKind::NotFound
            {
                panic!("remove {}: {error}", expectation.staged.display());
            }
        }
    }

    /// Write each staged actual output over the `.stderr` it belongs
    /// to, returning the `.stderr`s replaced. A fixture whose output
    /// matched its `.stderr` staged nothing, and is left alone.
    pub(crate) fn record(&self) -> Vec<&Path> {
        let mut recorded = Vec::new();
        for expectation in &self.0 {
            let text = match fs::read_to_string(&expectation.staged) {
                Ok(text) => text,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => panic!("read {}: {error}", expectation.staged.display()),
            };
            build_base::check_expectation(&expectation.committed, &text);
            fs::write(&expectation.committed, text).unwrap_or_else(|error| {
                panic!("write {}: {error}", expectation.committed.display())
            });
            recorded.push(expectation.committed.as_path());
        }
        recorded
    }
}

#[cfg(test)]
mod tests;
