//! The directory compiletest writes a run's output to, and what its
//! path does to the text that run is judged on.
//!
//! `dylint_testing` leaves compiletest's `build_base` at compiletest's
//! own default, [`env::temp_dir`], and offers no setter for it, so the
//! directory is wherever `TMPDIR` points. compiletest substitutes
//! [`SUBSTITUTION`] for that path throughout a fixture's rendered
//! output, the echoed source lines included.
//!
//! Of the paths compiletest substitutes out of an output, that one is
//! the only one able to swallow text a fixture meant to keep. The
//! others stand for paths that are machine-specific to begin with:
//! the fixture copy's own directory becomes `$DIR` and the manifest
//! directory becomes `$SRC_DIR`, and each is a long absolute path no
//! fixture holds by accident. A temp directory is short and ordinary,
//! `/tmp` above all, so a fixture about something else can hold one,
//! and is then judged on text that differs from what it was written
//! as, by however much `TMPDIR` differs from one machine to the next.
//! Under `PERFECTIONIST_BLESS` that text is recorded rather than
//! reported.
//!
//! Nothing here pins the directory, because nothing can:
//! `dylint_testing::ui::Test` exposes no `build_base` setter, and
//! setting `TMPDIR` from inside the process is unsafe in edition 2024
//! and races the threads a test binary runs its tests on. So
//! [`check_fixtures`] refuses a run whose fixtures would be judged on
//! rewritten text, and [`check_expectation`] refuses an expectation
//! that holds the mark of one.
//!
//! [`check_fixtures`] and [`check_expectation`] answer the question
//! from either end, which is what lets each stay simple.
//! [`check_fixtures`] reads the fixtures compiletest names as tests,
//! so it names the file to fix for the mistake that is actually made,
//! a temp path written into a fixture. It does not read an
//! `auxiliary/` crate, whose spans reach a fixture's output all the
//! same; [`check_expectation`] covers that from the other side, since
//! whatever carried the path, the output holds [`SUBSTITUTION`].

use crate::ui_fixtures::fixture_sources;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// What compiletest substitutes for [`path`] throughout an output.
const SUBSTITUTION: &str = "$TEST_BUILD_DIR";

/// What [`env::temp_dir`] falls back to with `TMPDIR` unset.
const UNSET_TMPDIR: &str = "/tmp";

/// Where compiletest writes this run's output, and so where
/// [`crate::bless`] reads each staged actual output from.
pub(crate) fn path() -> PathBuf {
    env::temp_dir()
}

/// The temp directories a fixture must not hold the path of: the one
/// this run writes to, and [`UNSET_TMPDIR`].
///
/// The fallback is checked even where it is not this run's directory,
/// so that a fixture holding `/tmp` is refused under the `test` recipe
/// too. That recipe points `TMPDIR` at a subdirectory of its own, and
/// would otherwise accept a fixture that fails for whoever runs
/// `cargo test` with `TMPDIR` unset.
fn temp_dirs() -> [PathBuf; 2] {
    [path(), PathBuf::from(UNSET_TMPDIR)]
}

/// Refuse the run if a fixture under `dir` holds the path of a temp
/// directory, or if the `.stderr` committed beside one is an
/// expectation [`check_expectation`] rejects.
pub(crate) fn check_fixtures(dir: &Path) {
    check_fixtures_against(dir, &temp_dirs());
}

/// [`check_fixtures`] with the temp directories given rather than read
/// from the environment.
fn check_fixtures_against(dir: &Path, temp_dirs: &[PathBuf]) {
    for fixture in fixture_sources(dir) {
        let source = fs::read_to_string(&fixture).expect("read a fixture .rs");
        for temp_dir in temp_dirs {
            let temp_dir = temp_dir.display().to_string();
            assert!(
                !source.contains(&temp_dir),
                "{}: a fixture must not hold the path of a temp directory, and this one holds \
                 {temp_dir:?}. compiletest substitutes `{SUBSTITUTION}` for the temp directory \
                 of the run throughout a fixture's rendered output, the echoed source lines \
                 included, so wherever that directory is the one written here, this fixture is \
                 judged on text it was not written as. Give it a path that is nobody's temp \
                 directory.",
                fixture.display(),
            );
        }
        let committed = fixture.with_extension("stderr");
        // A fixture that expects no diagnostics commits no `.stderr`.
        match fs::read_to_string(&committed) {
            Ok(text) => check_expectation(&committed, &text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("read {}: {error}", committed.display()),
        }
    }
}

/// Refuse `text` as the expectation committed at `committed`.
///
/// The text may be one a blessing run is about to write or one already
/// in the tree. Refusing both is what keeps an expectation that reads
/// differently from one machine to the next out of the repository,
/// whichever way it was produced.
pub(crate) fn check_expectation(committed: &Path, text: &str) {
    assert!(
        !text.contains(SUBSTITUTION),
        "{}: an expectation must not hold `{SUBSTITUTION}`. compiletest writes that only where \
         a rendered output held the temp directory of the run, so this text is the output of the \
         one machine whose temp directory was rewritten out of it. Take the temp directory's \
         path out of the fixture that produced it.",
        committed.display(),
    );
}

#[cfg(test)]
mod tests;
