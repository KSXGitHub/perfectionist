//! Test-support building blocks for perfectionist's integration
//! tests. These helpers materialise a minimal Cargo project on disk
//! and shell out to `cargo dylint` against it. Path inputs are taken
//! as parameters rather than discovered, because this crate is built
//! in isolation from any test workspace and has no access to the
//! caller's `CARGO_TARGET_DIR` or `CARGO_MANIFEST_DIR`.

use std::path::Path;

pub use tempfile::TempDir;

pub mod dylint;
pub mod manifest;
pub mod project;

pub use dylint::run_dylint;
pub use manifest::{
    DylintLibrary, DylintMetadata, DylintWorkspaceMetadata, fixture_cargo_toml, fixture_dylint_toml,
};
pub use project::build_project;

/// Materialise a fixture project in a fresh `TempDir`, run
/// `cargo dylint --all` against it (sharing the warmed `target/`), and
/// return the `TempDir` guard, the stderr output, and the success
/// flag. The `TempDir` is yielded first so the caller keeps the
/// project on disk for the duration of its assertions.
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
