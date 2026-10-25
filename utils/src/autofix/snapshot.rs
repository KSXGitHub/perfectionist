//! Read a codebase fixture off disk, and write one back.
//!
//! A snapshot is the whole fixture as text, keyed by the path each
//! file sits at relative to the fixture root, with `/` separators on
//! every platform so a stored diff reads the same everywhere. It is
//! what the harness diffs a round against, what it dumps when a round
//! fails, and what it materialises a fresh fixture from.
//!
//! Two paths are left out of every snapshot, each because the round
//! does not own it. `Cargo.lock` is the resolver's, and `dylint.toml`
//! carries the configuration under test plus an absolute path to this
//! checkout, so neither belongs in a diff a reader is asked to
//! review.

use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// A whole codebase fixture: relative path to file contents.
pub type Snapshot = BTreeMap<String, String>;

/// The paths a snapshot never carries, relative to the fixture root.
const EXCLUDED: &[&str] = &["Cargo.lock", "dylint.toml"];

/// Read every file under `root` into a [`Snapshot`].
///
/// Every file has to be UTF-8 and has to end in a newline: the diff
/// format spells both assumptions, and a fixture that breaks either
/// would be reported as a change on every round.
pub fn read(root: &Path) -> Snapshot {
    let mut snapshot = Snapshot::new();
    collect(root, "", &mut snapshot);
    snapshot
}

/// Walk `dir`, whose path relative to the snapshot root is `prefix`,
/// adding each file it holds to `snapshot`.
fn collect(dir: &Path, prefix: &str, snapshot: &mut Snapshot) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()));
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|error| panic!("read an entry of {}: {error}", dir.display()));
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = match prefix.is_empty() {
            true => name,
            false => format!("{prefix}/{name}"),
        };
        if EXCLUDED.contains(&path.as_str()) {
            continue;
        }
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("stat {}: {error}", entry.path().display()));
        match file_type.is_dir() {
            true => collect(&entry.path(), &path, snapshot),
            false => {
                snapshot.insert(path.clone(), read_text(&entry.path(), &path));
            }
        }
    }
}

/// One file's contents, held to the two assumptions the diff format
/// makes about them.
fn read_text(file: &Path, path: &str) -> String {
    let contents =
        fs::read(file).unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
    let contents = String::from_utf8(contents)
        .unwrap_or_else(|error| panic!("{path} is not UTF-8: {error}; a fixture has to be text"));
    assert!(
        contents.is_empty() || contents.ends_with('\n'),
        "{path} does not end in a newline; every fixture file has to, \
         since the diff format carries no marker for a file that does not",
    );
    contents
}

/// Materialise `snapshot` under `root`, creating the directories it
/// implies. Anything already there and not named by the snapshot is
/// left alone, so a caller may lay a snapshot over a tree that
/// already holds the generated `dylint.toml`.
pub fn write(root: &Path, snapshot: &Snapshot) {
    let entries: BTreeMap<String, FileSystemTree<String, String>> = snapshot
        .iter()
        .map(|(path, contents)| (path.clone(), FileSystemTree::File(contents.clone())))
        .collect();
    MergeableFileSystemTree::from(FileSystemTree::Directory(entries))
        .build(root)
        .unwrap_or_else(|error| panic!("materialise a snapshot at {}: {error}", root.display()));
}
