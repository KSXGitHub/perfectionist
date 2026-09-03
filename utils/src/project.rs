//! Materialise a fixture Cargo project on disk: write its
//! `Cargo.toml`, `dylint.toml`, and the supplied source files into
//! a target directory.

use crate::manifest::{fixture_cargo_toml, fixture_dylint_toml};
use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};
use std::collections::BTreeMap;
use std::path::Path;

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
    build_project_with_config(project_dir, package_name, perfectionist_dir, sources, "");
}

/// Like [`build_project`], but appends `extra_dylint_toml` to the
/// generated `dylint.toml`. Use it to add a per-rule
/// `["perfectionist::<rule>"]` configuration table next to the library
/// metadata — library discovery only reads `workspace.metadata.dylint`,
/// so unrelated top-level tables are ignored by it while still being
/// visible to `dylint_linting::config_or_default`. Quote the `::` key,
/// since a bare `perfectionist::<rule>` is invalid TOML.
pub fn build_project_with_config(
    project_dir: &Path,
    package_name: &str,
    perfectionist_dir: &Path,
    sources: &[(&str, &str)],
    extra_dylint_toml: &str,
) {
    let base_dylint_toml = fixture_dylint_toml(perfectionist_dir);
    let dylint_toml = match extra_dylint_toml {
        "" => base_dylint_toml,
        extra => format!("{base_dylint_toml}\n{extra}"),
    };
    let entries: BTreeMap<String, FileSystemTree<String, String>> = [
        (
            "Cargo.toml".to_owned(),
            FileSystemTree::File(fixture_cargo_toml(package_name)),
        ),
        ("dylint.toml".to_owned(), FileSystemTree::File(dylint_toml)),
    ]
    .into_iter()
    .chain(sources.iter().map(|(path, contents)| {
        (
            (*path).to_owned(),
            FileSystemTree::File((*contents).to_owned()),
        )
    }))
    .collect();
    let tree = MergeableFileSystemTree::from(FileSystemTree::Directory(entries));
    tree.build(project_dir)
        .expect("failed to materialise project tree");
}
