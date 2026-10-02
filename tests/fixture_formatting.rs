//! Assert that rustfmt is disabled over the fixture trees, and over
//! nothing else.
//!
//! A fixture's written layout is what the lint under test reads, so
//! reformatting one can leave it testing nothing: rustfmt collapses a
//! deliberately multi-line macro call onto one line, sorts a `use`
//! block whose order is the violation, and strips the trailing comma a
//! multi-line `#![allow(...)]` case is built around. Even where the
//! diagnostic survives, the rewrapping rewrites the source lines the
//! committed `.stderr` quotes back. A `.fixed.rs` fares worse still,
//! because it has to match what rustc's suggestion machinery writes
//! out, which is not rustfmt's output.
//!
//! `cargo fmt` never reaches these files, because nothing declares
//! them as modules of any crate in the workspace. An editor does
//! reach them. rust-analyzer pipes the buffer of whatever `.rs` file
//! is open to rustfmt on standard input, with the file's own directory
//! as the working directory, and rustfmt reads its configuration by
//! walking up from there. So a `rustfmt.toml` at each fixture root
//! setting `disable_all_formatting` is what keeps format-on-save off a
//! fixture. The same file also covers `rustfmt <path>`, which resolves
//! its configuration from the path's directory.
//!
//! Note what does *not* work, so it is not reached for instead:
//! rustfmt's `ignore` list matches against the input's path, and an
//! editor's input arrives on standard input with no path, so a list at
//! the repository root never matches the buffer being saved.
//!
//! Two ways to lose the shield are silent, so each gets a test.
//! rustfmt reads the *nearest* configuration file and merges nothing
//! into it, so a second `rustfmt.toml` deeper inside a fixture tree
//! re-enables formatting for everything under it, whatever it was
//! added for. And a shield that drifts over real source would quietly
//! stop `cargo fmt` from reaching that source at all.

use std::fs;
use std::path::{Path, PathBuf};

/// The fixture trees, relative to the crate root. Every `.rs` file
/// under one of these is an input to the lint suite rather than source
/// to keep formatted.
const FIXTURE_ROOTS: &[&str] = &["ui", "ui-toml", "tests/fixtures"];

/// Directory names the walk does not enter. Each holds generated
/// output or tool state rather than committed files, and `target/`
/// alone is large enough to dominate the walk.
const GENERATED_DIRS: &[&str] = &[
    ".cache",
    ".dev-tools",
    ".git",
    "gh-pages",
    "node_modules",
    "target",
];

/// The names rustfmt accepts for a configuration file, in no
/// particular order: this module asks whether *every* one present in a
/// directory disables formatting, which is the same answer however
/// rustfmt breaks the tie between two of them in one directory.
const CONFIG_FILE_NAMES: &[&str] = &["rustfmt.toml", ".rustfmt.toml"];

#[test]
fn every_fixture_file_is_shielded_from_rustfmt() {
    let root = crate_root();
    let exposed = rust_files(&root)
        .into_iter()
        .filter(|file| is_fixture(file, &root) && !is_shielded(file, &root))
        .collect::<Vec<_>>();
    let report = relative_to(&exposed, &root);
    assert!(
        exposed.is_empty(),
        "these fixtures would be reformatted on save; the nearest rustfmt \
         configuration above them does not set `disable_all_formatting`:\n{report}",
    );
}

#[test]
fn nothing_outside_the_fixture_trees_is_shielded_from_rustfmt() {
    let root = crate_root();
    let shielded = rust_files(&root)
        .into_iter()
        .filter(|file| !is_fixture(file, &root) && is_shielded(file, &root))
        .collect::<Vec<_>>();
    let report = relative_to(&shielded, &root);
    assert!(
        shielded.is_empty(),
        "these files are source, not fixtures, and a rustfmt configuration \
         above them sets `disable_all_formatting`, so `cargo fmt` leaves them \
         unformatted:\n{report}",
    );
}

/// The repository root, which is this test binary's manifest
/// directory.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file in the repository, skipping [`GENERATED_DIRS`].
fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect_rust_files(root, &mut found);
    found.sort();
    found
}

fn collect_rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read a directory of the repository") {
        let entry = entry.expect("read a repository directory entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("repository entry file type");
        if file_type.is_dir() {
            let name = entry.file_name();
            if !GENERATED_DIRS.iter().any(|skipped| name == **skipped) {
                collect_rust_files(&path, found);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// Whether `file` sits under one of [`FIXTURE_ROOTS`].
fn is_fixture(file: &Path, root: &Path) -> bool {
    FIXTURE_ROOTS
        .iter()
        .any(|fixture_root| file.starts_with(root.join(fixture_root)))
}

/// Whether the rustfmt configuration that governs `file` disables
/// formatting. rustfmt reads the first configuration file it finds
/// walking up from the file's own directory and merges nothing from
/// further up, so the walk stops there too, and a directory with no
/// configuration file at all leaves formatting on.
fn is_shielded(file: &Path, root: &Path) -> bool {
    let mut dir = file.parent().expect("a file has a parent directory");
    loop {
        let configs = CONFIG_FILE_NAMES
            .iter()
            .map(|name| dir.join(name))
            .filter(|config| config.is_file())
            .collect::<Vec<_>>();
        if !configs.is_empty() {
            return configs.iter().all(|config| disables_formatting(config));
        }
        if dir == root {
            return false;
        }
        dir = dir.parent().expect("the walk stops at the crate root");
    }
}

/// Whether `config` sets `disable_all_formatting = true`.
fn disables_formatting(config: &Path) -> bool {
    let text = fs::read_to_string(config).expect("read a rustfmt configuration file");
    let table = text
        .parse::<toml::Table>()
        .expect("parse a rustfmt configuration file");
    table
        .get("disable_all_formatting")
        .and_then(toml::Value::as_bool)
        == Some(true)
}

/// The paths as a newline-separated list, each relative to `root`, for
/// an assertion message.
fn relative_to(paths: &[PathBuf], root: &Path) -> String {
    paths
        .iter()
        .map(|path| format!("  {}", path.strip_prefix(root).unwrap_or(path).display()))
        .collect::<Vec<_>>()
        .join("\n")
}
