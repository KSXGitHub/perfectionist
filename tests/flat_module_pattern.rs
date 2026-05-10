//! Integration test for `flat_module_pattern`.
//!
//! Each test materialises a minimal Cargo project under a `TempDir`,
//! points its `dylint.toml` at the perfectionist crate this test
//! ships with, and runs `cargo dylint --all` against the project to
//! observe what the lint actually emits in a real (non-`compiletest`)
//! build.

use std::{collections::BTreeMap, path::Path, process::Command};

use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};
use cargo_toml::{Edition, Inheritable, Manifest, Package, Product, Workspace};
use command_extra::CommandExtra;
use pipe_trait::Pipe;
use serde::Serialize;
use tempfile::TempDir;

const PERFECTIONIST_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Default, Serialize)]
struct DylintWorkspaceMetadata {
    dylint: DylintMetadata,
}

#[derive(Default, Serialize)]
struct DylintMetadata {
    libraries: Vec<DylintLibrary>,
}

#[derive(Default, Serialize)]
struct DylintLibrary {
    path: String,
}

fn fixture_cargo_toml() -> String {
    let mut package = Package::<()>::new("fixture", "0.0.0");
    package.edition = Inheritable::Set(Edition::E2024);
    let manifest = Manifest::<()> {
        package: Some(package),
        lib: Some(Product {
            path: Some("src/lib.rs".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    toml::to_string(&manifest).expect("serialize Cargo.toml")
}

fn fixture_dylint_toml() -> String {
    let manifest = Manifest::<DylintWorkspaceMetadata> {
        workspace: Some(Workspace {
            metadata: Some(DylintWorkspaceMetadata {
                dylint: DylintMetadata {
                    libraries: vec![DylintLibrary {
                        path: PERFECTIONIST_DIR.to_owned(),
                    }],
                },
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    toml::to_string(&manifest).expect("serialize dylint.toml")
}

fn run_dylint(project_dir: &Path) -> (String, bool) {
    let output = "cargo"
        .pipe(Command::new)
        .with_arg("dylint")
        .with_arg("--all")
        .with_current_dir(project_dir)
        .output()
        .expect("failed to run `cargo dylint`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}

/// Materialise a project with `Cargo.toml`, `dylint.toml`, and the
/// given `(relative_path, contents)` source files, then run
/// `cargo dylint --all` and return `(stderr, success)`. The `TempDir`
/// is returned alongside so it stays alive for the caller's assertions
/// (e.g. when inspecting paths it embedded).
fn run_project_with_sources(sources: &[(&str, &str)]) -> (TempDir, String, bool) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let mut entries: BTreeMap<String, FileSystemTree<String, String>> = BTreeMap::new();
    entries.insert(
        "Cargo.toml".to_owned(),
        FileSystemTree::File(fixture_cargo_toml()),
    );
    entries.insert(
        "dylint.toml".to_owned(),
        FileSystemTree::File(fixture_dylint_toml()),
    );
    for (path, contents) in sources {
        entries.insert(
            (*path).to_owned(),
            FileSystemTree::File((*contents).to_owned()),
        );
    }
    let tree = MergeableFileSystemTree::from(FileSystemTree::Directory(entries));
    tree.build(temp.path())
        .expect("failed to materialise project tree");
    let (stderr, success) = run_dylint(temp.path());
    (temp, stderr, success)
}

#[test]
fn flags_mod_rs_submodule() {
    let (_temp, stderr, success) = run_project_with_sources(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo/mod.rs", "pub fn bar() {}\n"),
    ]);
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("perfectionist::flat_module_pattern"),
        "expected `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("submodule uses the `mod.rs` layout"),
        "expected lint message; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("foo/mod.rs"),
        "expected reference to `foo/mod.rs`; stderr was:\n{stderr}"
    );
}

#[test]
fn does_not_flag_flat_layout() {
    let (_temp, stderr, success) = run_project_with_sources(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub fn bar() {}\n"),
    ]);
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains("perfectionist::flat_module_pattern"),
        "did not expect `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
}

#[test]
fn does_not_flag_deep_flat_layout() {
    let (_temp, stderr, success) = run_project_with_sources(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub mod bar;\n"),
        ("src/foo/bar.rs", "pub mod baz;\n"),
        ("src/foo/bar/baz.rs", ""),
    ]);
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains("perfectionist::flat_module_pattern"),
        "did not expect `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
}

#[test]
fn flags_deep_mod_rs_at_leaf_level() {
    let (_temp, stderr, success) = run_project_with_sources(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub mod bar;\n"),
        ("src/foo/bar/mod.rs", "pub mod baz;\n"),
        ("src/foo/bar/baz.rs", ""),
    ]);
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("perfectionist::flat_module_pattern"),
        "expected `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("foo/bar/mod.rs"),
        "expected reference to `foo/bar/mod.rs`; stderr was:\n{stderr}"
    );
}

#[test]
fn flags_mod_rs_at_intermediate_level() {
    let (_temp, stderr, success) = run_project_with_sources(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo/mod.rs", "pub mod bar;\n"),
        ("src/foo/bar.rs", "pub mod baz;\n"),
        ("src/foo/bar/baz.rs", ""),
    ]);
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("perfectionist::flat_module_pattern"),
        "expected `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("foo/mod.rs"),
        "expected reference to `foo/mod.rs`; stderr was:\n{stderr}"
    );
}
