//! Materialise a fixture Cargo project on disk: write its
//! `Cargo.toml`, `dylint.toml`, and the supplied source files into
//! a target directory.

use std::collections::BTreeMap;
use std::path::Path;

use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};

use crate::manifest::{fixture_cargo_toml, fixture_dylint_toml};

/// Materialise a Cargo project at `project_dir` consisting of a
/// `Cargo.toml` for a library package named `package_name`, a
/// `dylint.toml` pointing at the perfectionist crate, and the given
/// `(relative_path, contents)` source files.
pub fn build_project(
    project_dir: &Path,
    package_name: &str,
    perfectionist_dir: &Path,
    sources: &[(&str, &str)],
) {
    let mut entries: BTreeMap<String, FileSystemTree<String, String>> = BTreeMap::new();
    entries.insert(
        "Cargo.toml".to_owned(),
        FileSystemTree::File(fixture_cargo_toml(package_name)),
    );
    entries.insert(
        "dylint.toml".to_owned(),
        FileSystemTree::File(fixture_dylint_toml(perfectionist_dir)),
    );
    for (path, contents) in sources {
        entries.insert(
            (*path).to_owned(),
            FileSystemTree::File((*contents).to_owned()),
        );
    }
    let tree = MergeableFileSystemTree::from(FileSystemTree::Directory(entries));
    tree.build(project_dir)
        .expect("failed to materialise project tree");
}
