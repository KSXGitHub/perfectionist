//! Materialise a fixture Cargo project on disk: write its
//! `Cargo.toml`, `dylint.toml`, and the supplied source files into
//! a target directory.

use std::{collections::BTreeMap, path::Path};

use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};

use crate::manifest::{fixture_cargo_toml, fixture_dylint_toml_with_config};

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
    build_project_with_dylint_config(project_dir, package_name, perfectionist_dir, sources, "");
}

/// Like [`build_project`], but appends `dylint_toml_extra` to the
/// generated `dylint.toml` — typically a per-rule configuration table.
pub fn build_project_with_dylint_config(
    project_dir: &Path,
    package_name: &str,
    perfectionist_dir: &Path,
    sources: &[(&str, &str)],
    dylint_toml_extra: &str,
) {
    let mut entries: BTreeMap<String, FileSystemTree<String, String>> = BTreeMap::new();
    entries.insert(
        "Cargo.toml".to_owned(),
        FileSystemTree::File(fixture_cargo_toml(package_name)),
    );
    entries.insert(
        "dylint.toml".to_owned(),
        FileSystemTree::File(fixture_dylint_toml_with_config(
            perfectionist_dir,
            dylint_toml_extra,
        )),
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
