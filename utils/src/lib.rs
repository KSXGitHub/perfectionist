use std::{collections::BTreeMap, path::Path, process::Command};

use build_fs_tree::{Build, FileSystemTree, MergeableFileSystemTree};
use cargo_toml::{Edition, Inheritable, Manifest, Package, Product, Workspace};
use command_extra::CommandExtra;
use pipe_trait::Pipe;
use serde::Serialize;

pub use tempfile::TempDir;

#[derive(Default, Serialize)]
pub struct DylintWorkspaceMetadata {
    pub dylint: DylintMetadata,
}

#[derive(Default, Serialize)]
pub struct DylintMetadata {
    pub libraries: Vec<DylintLibrary>,
}

#[derive(Default, Serialize)]
pub struct DylintLibrary {
    pub path: String,
}

pub fn fixture_cargo_toml(package_name: &str) -> String {
    let mut package = Package::<()>::new(package_name, "0.0.0");
    package.edition = Inheritable::Set(Edition::E2024);
    let manifest = Manifest::<()> {
        package: Some(package),
        lib: Some(Product {
            path: Some("src/lib.rs".to_owned()),
            ..Default::default()
        }),
        // Declare an empty workspace so cargo doesn't walk up the
        // filesystem and try to enroll the fixture into the
        // perfectionist workspace it happens to be nested inside.
        workspace: Some(Workspace::default()),
        ..Default::default()
    };
    toml::to_string(&manifest).expect("serialize Cargo.toml")
}

pub fn fixture_dylint_toml(perfectionist_dir: &Path) -> String {
    let manifest = Manifest::<DylintWorkspaceMetadata> {
        workspace: Some(Workspace {
            metadata: Some(DylintWorkspaceMetadata {
                dylint: DylintMetadata {
                    libraries: vec![DylintLibrary {
                        path: perfectionist_dir.display().to_string(),
                    }],
                },
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    toml::to_string(&manifest).expect("serialize dylint.toml")
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

/// Run `cargo dylint --all` inside `project_dir`, with
/// `CARGO_TARGET_DIR` pointed at `shared_target_dir` so the build
/// artefacts are reused across invocations.
pub fn run_dylint(project_dir: &Path, shared_target_dir: &Path) -> (String, bool) {
    let output = "cargo"
        .pipe(Command::new)
        .with_arg("dylint")
        .with_arg("--all")
        .with_current_dir(project_dir)
        .with_env("CARGO_TARGET_DIR", shared_target_dir)
        .output()
        .expect("failed to run `cargo dylint`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}

/// Materialise a fixture project in a fresh `TempDir`, run
/// `cargo dylint --all` against it (sharing the warmed `target/`), and
/// return the stderr output, success flag, and the `TempDir` guard so
/// the caller keeps the project on disk for the duration of its
/// assertions.
pub fn run_project_with_sources(
    package_name: &str,
    perfectionist_dir: &Path,
    shared_target_dir: &Path,
    sources: &[(&str, &str)],
) -> (TempDir, String, bool) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project(temp.path(), package_name, perfectionist_dir, sources);
    let (stderr, success) = run_dylint(temp.path(), shared_target_dir);
    (temp, stderr, success)
}
