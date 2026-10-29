use super::{check_fixtures_against, temp_dirs};
use crate::TempDir;
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Write a fixture at `<root>/ui/<name>.rs` holding `source`, plus the
/// `.stderr` beside it where an expectation is given, returning the
/// directory to check.
fn tree(root: &Path, name: &str, source: &str, expectation: Option<&str>) -> PathBuf {
    let dir = root.join("ui");
    fs::create_dir_all(&dir).expect("create a fixture dir");
    fs::write(dir.join(format!("{name}.rs")), source).expect("write a fixture .rs");
    if let Some(expectation) = expectation {
        fs::write(dir.join(format!("{name}.stderr")), expectation)
            .expect("write a fixture .stderr");
    }
    dir
}

/// The run's own directory alone would let the `test` recipe accept a
/// fixture holding `/tmp`, which fails for whoever runs `cargo test`
/// with `TMPDIR` unset.
#[test]
fn the_checked_directories_are_the_runs_and_the_unset_default() {
    assert_eq!(temp_dirs(), [env::temp_dir(), PathBuf::from("/tmp")]);
}

#[test]
fn a_tree_holding_no_temp_path_passes() {
    let temp = TempDir::new().expect("create a tree");
    let dir = tree(
        temp.path(),
        "clean",
        "fn main() {}\n",
        Some("warning: N warnings emitted\n"),
    );
    check_fixtures_against(&dir, &[PathBuf::from("/tmp")]);
}

/// A fixture that expects no diagnostics commits no `.stderr`, which
/// is not an expectation to reject.
#[test]
fn a_fixture_without_an_expectation_passes() {
    let temp = TempDir::new().expect("create a tree");
    let dir = tree(temp.path(), "silent", "fn main() {}\n", None);
    check_fixtures_against(&dir, &[PathBuf::from("/tmp")]);
}

#[test]
#[should_panic(expected = "a fixture must not hold the path of a temp directory")]
fn a_fixture_holding_a_temp_path_is_refused() {
    let temp = TempDir::new().expect("create a tree");
    let dir = tree(
        temp.path(),
        "collides",
        "fn main() {\n    let _ = \"/tmp\";\n}\n",
        None,
    );
    check_fixtures_against(&dir, &[PathBuf::from("/tmp")]);
}

/// The path is matched as a substring, exactly as compiletest
/// substitutes it, so a path under the temp directory collides and not
/// only the directory itself.
#[test]
#[should_panic(expected = "a fixture must not hold the path of a temp directory")]
fn a_fixture_holding_a_path_under_a_temp_directory_is_refused() {
    let temp = TempDir::new().expect("create a tree");
    let dir = tree(
        temp.path(),
        "nested",
        "fn main() {\n    let _ = \"/tmp/scratch/held\";\n}\n",
        None,
    );
    check_fixtures_against(&dir, &[PathBuf::from("/tmp")]);
}

/// The source here holds no path, so the refusal is the expectation
/// arm's rather than the one that reads the fixture.
#[test]
#[should_panic(expected = "an expectation must not hold `$TEST_BUILD_DIR`")]
fn a_committed_expectation_holding_the_substitution_is_refused() {
    let temp = TempDir::new().expect("create a tree");
    let dir = tree(
        temp.path(),
        "recorded",
        "fn main() {}\n",
        Some("LL |     let _ = \"$TEST_BUILD_DIR\";\n"),
    );
    check_fixtures_against(&dir, &[PathBuf::from("/tmp")]);
}
