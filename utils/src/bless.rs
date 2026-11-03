//! Re-record the `.stderr` fixtures from what the run produced.
//!
//! Where compiletest writes a fixture's actual output is promised by
//! no API: the path here is built from compiletest's own defaults,
//! which `dylint_testing` leaves alone.
//!
//! compiletest never deletes one of those files, so a stale one has to
//! go before the run rather than after.

use crate::ui_fixtures::patch::write::unified;
use crate::ui_fixtures::{fixture_sources, runs_rustfix, strip_directives};
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
    /// The file the repository commits.
    committed: PathBuf,
    /// Where compiletest writes what the run produced, on a mismatch.
    staged: PathBuf,
    /// How the staged text becomes the committed text.
    recording: Recording,
}

/// What a staged file holds.
enum Recording {
    /// The diagnostics, which the `.stderr` holds as they stand.
    Diagnostics,
    /// The fixed source, which the `.diff` holds as its difference
    /// from the fixture at this path.
    Fixed { source: PathBuf },
}

impl Expectations {
    /// The expectations for the fixtures under
    /// `<manifest_dir>/<relative>`.
    pub(crate) fn of(manifest_dir: &Path, relative: &Path) -> Self {
        Expectations::under(manifest_dir, relative, &env::temp_dir())
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
        let mut expectations = Vec::new();
        for fixture in fixture_sources(&manifest_dir.join(relative)) {
            let below = fixture
                .strip_prefix(&root)
                .expect("a fixture sits under its own tree");
            let staged = |extension: &str| {
                staged_root
                    .join(below)
                    .with_extension(format!("{STAGE_ID}.{extension}"))
            };
            if runs_rustfix(&read(&fixture)) {
                expectations.push(Expectation {
                    committed: fixture.with_extension("diff"),
                    staged: staged("fixed"),
                    recording: Recording::Fixed {
                        source: fixture.clone(),
                    },
                });
            }
            expectations.push(Expectation {
                committed: fixture.with_extension("stderr"),
                staged: staged("stderr"),
                recording: Recording::Diagnostics,
            });
        }
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

    /// Write each staged output to the file it belongs to, returning
    /// the files replaced.
    pub(crate) fn record(&self) -> Vec<&Path> {
        let mut recorded = Vec::new();
        for expectation in &self.0 {
            if !expectation.staged.exists() {
                continue;
            }
            let text = match &expectation.recording {
                Recording::Diagnostics => read(&expectation.staged),
                Recording::Fixed { source } => {
                    let name = source
                        .file_name()
                        .expect("a fixture has a file name")
                        .to_str()
                        .expect("a fixture's name is UTF-8");
                    let staged = read(&expectation.staged);
                    unified(name, &read(source), strip_directives(&staged))
                }
            };
            let committed = &expectation.committed;
            fs::write(committed, text)
                .unwrap_or_else(|error| panic!("write {}: {error}", committed.display()));
            recorded.push(committed.as_path());
        }
        recorded
    }
}

fn read(file: &Path) -> String {
    fs::read_to_string(file).unwrap_or_else(|error| panic!("read {}: {error}", file.display()))
}

#[cfg(test)]
mod tests;
