//! Integration test for `flat_module_pattern`.
//!
//! Each test materialises a minimal Cargo project under a `TempDir`,
//! points its `dylint.toml` at the perfectionist crate this test
//! ships with, and runs `cargo dylint --all` against the project to
//! observe what the lint actually emits in a real (non-`compiletest`)
//! build.

use std::{path::Path, process::Command};

use build_fs_tree::{Build, MergeableFileSystemTree, dir, file};
use command_extra::CommandExtra;
use pipe_trait::Pipe;
use serde::Serialize;
use tempfile::TempDir;

const PERFECTIONIST_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Serialize)]
struct CargoToml {
    package: Package,
    lib: Lib,
}

#[derive(Serialize)]
struct Package {
    name: &'static str,
    version: &'static str,
    edition: &'static str,
}

#[derive(Serialize)]
struct Lib {
    path: &'static str,
}

#[derive(Serialize)]
struct DylintToml {
    workspace: Workspace,
}

#[derive(Serialize)]
struct Workspace {
    metadata: WorkspaceMetadata,
}

#[derive(Serialize)]
struct WorkspaceMetadata {
    dylint: DylintMetadata,
}

#[derive(Serialize)]
struct DylintMetadata {
    libraries: Vec<DylintLibrary>,
}

#[derive(Serialize)]
struct DylintLibrary {
    path: String,
}

fn fixture_cargo_toml() -> String {
    let manifest = CargoToml {
        package: Package {
            name: "fixture",
            version: "0.0.0",
            edition: "2024",
        },
        lib: Lib { path: "src/lib.rs" },
    };
    toml::to_string(&manifest).expect("serialize Cargo.toml")
}

fn fixture_dylint_toml() -> String {
    let config = DylintToml {
        workspace: Workspace {
            metadata: WorkspaceMetadata {
                dylint: DylintMetadata {
                    libraries: vec![DylintLibrary {
                        path: PERFECTIONIST_DIR.to_owned(),
                    }],
                },
            },
        },
    };
    toml::to_string(&config).expect("serialize dylint.toml")
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

#[test]
fn flags_mod_rs_submodule() {
    let temp = TempDir::new().expect("failed to create temp dir");
    let tree = MergeableFileSystemTree::<&str, String>::from(dir! {
        "Cargo.toml" => file!(fixture_cargo_toml())
        "dylint.toml" => file!(fixture_dylint_toml())
        "src/lib.rs" => file!("pub mod foo;\n".to_owned())
        "src/foo/mod.rs" => file!("pub fn bar() {}\n".to_owned())
    });
    tree.build(temp.path())
        .expect("failed to materialise project tree");

    let (stderr, success) = run_dylint(temp.path());
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
    let temp = TempDir::new().expect("failed to create temp dir");
    let tree = MergeableFileSystemTree::<&str, String>::from(dir! {
        "Cargo.toml" => file!(fixture_cargo_toml())
        "dylint.toml" => file!(fixture_dylint_toml())
        "src/lib.rs" => file!("pub mod foo;\n".to_owned())
        "src/foo.rs" => file!("pub fn bar() {}\n".to_owned())
    });
    tree.build(temp.path())
        .expect("failed to materialise project tree");

    let (stderr, success) = run_dylint(temp.path());
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains("perfectionist::flat_module_pattern"),
        "did not expect `flat_module_pattern` warning; stderr was:\n{stderr}"
    );
}
