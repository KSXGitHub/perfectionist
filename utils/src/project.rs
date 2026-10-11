//! Materialise a fixture Cargo project on disk: write its
//! `Cargo.toml`, `dylint.toml`, and the supplied source files into
//! a target directory.

use crate::manifest::{fixture_cargo_toml, fixture_dylint_toml};
use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

/// The package names already materialised by this process.
///
/// Every fixture compiles into one shared target dir, and cargo keys a
/// unit's fingerprint on the package's name and version, not on where
/// its sources are. The version is the same for all of them, so the
/// name is the only thing holding two fixtures apart.
static TAKEN: LazyLock<Mutex<BTreeSet<String>>> = LazyLock::new(|| Mutex::new(BTreeSet::new()));

/// Claim `package_name` for one fixture, or panic because another
/// already holds it.
///
/// Two fixtures sharing a name share a fingerprint. Whichever cargo
/// reaches second is then told its unit is fresh — the check is on
/// mtimes, not on content — and replays the first one's diagnostics,
/// so a test asserts against another test's output. It passes or fails
/// on which fixture was written first, which is why this is a panic
/// rather than a comment: tests within one binary run concurrently,
/// and the reuse is invisible until the race lands.
fn claim(package_name: &str) {
    let fresh = TAKEN
        .lock()
        .expect("the package-name registry is poisoned")
        .insert(package_name.to_owned());
    assert!(
        fresh,
        "the fixture package name {package_name:?} is already taken in this test binary; \
         give this fixture its own name, since a shared one shares a build fingerprint",
    );
}

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
    claim(package_name);
    let mut entries: BTreeMap<String, FileSystemTree<String, String>> = BTreeMap::new();
    entries.insert(
        "Cargo.toml".to_owned(),
        FileSystemTree::File(fixture_cargo_toml(package_name)),
    );
    let mut dylint_toml = fixture_dylint_toml(perfectionist_dir);
    if !extra_dylint_toml.is_empty() {
        dylint_toml.push('\n');
        dylint_toml.push_str(extra_dylint_toml);
    }
    entries.insert("dylint.toml".to_owned(), FileSystemTree::File(dylint_toml));
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

#[cfg(test)]
mod tests;
