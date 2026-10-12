//! The climb to the workspace root, exercised against fixture trees in
//! a `TempDir`.
//!
//! Each case names the directory to start from, so none of them needs
//! `CARGO_MANIFEST_DIR`. Setting that variable would steer every other
//! reader in the test binary, and the cache the walk feeds answers its
//! first caller for the rest of the process.

use super::read_workspace_from;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// A manifest for a package named `name`. The name is what the
/// assertions read back, so every file in a fixture tree says which
/// one the walk settled on.
fn package(name: &str) -> String {
    format!("[package]\nname = {name:?}\n")
}

/// A manifest for a package named `name` that is also a workspace
/// root, which is what makes the walk stop at it.
fn workspace_root(name: &str) -> String {
    format!("{}\n[workspace]\nmembers = []\n", package(name))
}

/// Create `dir` and write `contents` as its `Cargo.toml`.
fn write_manifest(dir: &Path, contents: &str) {
    fs::create_dir_all(dir).expect("failed to create a fixture directory");
    fs::write(dir.join("Cargo.toml"), contents).expect("failed to write a fixture manifest");
}

/// The `package.name` of the manifest a walk from `dir` settles on.
fn found_package(dir: &Path) -> Option<String> {
    let manifest = read_workspace_from(dir)?;
    let name = manifest.get("package")?.get("name")?.as_str()?;
    Some(name.to_owned())
}

fn tree() -> TempDir {
    TempDir::new().expect("failed to create a temp dir")
}

#[test]
fn a_member_reads_the_root_above_it() {
    let tree = tree();
    write_manifest(tree.path(), &workspace_root("root"));
    let member = tree.path().join("member");
    write_manifest(&member, &package("member"));
    assert_eq!(found_package(&member).as_deref(), Some("root"));
}

/// "At or above": a crate rooted at the workspace root reads that same
/// manifest instead of climbing past it.
#[test]
fn the_root_is_read_from_its_own_directory() {
    let tree = tree();
    write_manifest(tree.path(), &workspace_root("root"));
    assert_eq!(found_package(tree.path()).as_deref(), Some("root"));
}

/// Nested workspaces: the climb stops at the first `[workspace]` it
/// meets, which is the one Cargo resolves a member of the inner
/// workspace against.
#[test]
fn the_nearest_workspace_above_wins() {
    let tree = tree();
    write_manifest(tree.path(), &workspace_root("outer"));
    let inner = tree.path().join("inner");
    write_manifest(&inner, &workspace_root("inner"));
    let member = inner.join("member");
    write_manifest(&member, &package("member"));
    assert_eq!(found_package(&member).as_deref(), Some("inner"));
}

#[test]
fn an_unparsable_manifest_does_not_hide_the_root() {
    let tree = tree();
    write_manifest(tree.path(), &workspace_root("root"));
    let broken = tree.path().join("broken");
    write_manifest(&broken, "[package\nname = ");
    let member = broken.join("member");
    write_manifest(&member, &package("member"));
    assert_eq!(found_package(&member).as_deref(), Some("root"));
}

/// `workspace = true` is how a member inherits a key, not how a
/// workspace is defined, so a manifest carrying it is not the root the
/// climb is looking for.
#[test]
fn a_workspace_key_that_is_not_a_table_is_stepped_over() {
    let tree = tree();
    write_manifest(tree.path(), &workspace_root("root"));
    let member = tree.path().join("member");
    write_manifest(
        &member,
        "workspace = true\n\n[package]\nname = \"member\"\n",
    );
    assert_eq!(found_package(&member).as_deref(), Some("root"));
}

/// A directory holding no `Cargo.toml` at all is nothing to read, and
/// the climb carries on above it.
#[test]
fn a_directory_without_a_manifest_is_climbed_past() {
    let tree = tree();
    write_manifest(tree.path(), &workspace_root("root"));
    let nested = tree.path().join("bare").join("deeper");
    fs::create_dir_all(&nested).expect("failed to create a fixture directory");
    assert_eq!(found_package(&nested).as_deref(), Some("root"));
}
