//! Test-support building blocks for perfectionist's test suites.
//! Most of these helpers materialise a minimal Cargo project on disk
//! and shell out to `cargo dylint` against it. Path inputs are taken
//! as parameters rather than discovered, because this crate is built
//! in isolation from any test workspace and has no access to the
//! caller's `CARGO_TARGET_DIR` or `CARGO_MANIFEST_DIR`.
//!
//! The exception is [`scratch`], which every test suite in the
//! repository — the gen-docs unit tests included — goes through to
//! keep its temporary files out of `/tmp` proper.

pub use tempfile::TempDir;

use std::path::Path;

pub mod dylint;
pub mod manifest;
pub mod project;
pub mod scratch;

pub use dylint::{run_dylint, run_dylint_all_targets};
pub use manifest::{
    DylintLibrary, DylintMetadata, DylintWorkspaceMetadata, fixture_cargo_toml, fixture_dylint_toml,
};
pub use project::{build_project, build_project_with_config};

/// Materialise a fixture project in a fresh [`TempDir`] under
/// [`scratch::root`], run `cargo dylint --all` against it (sharing
/// the warmed `target/`), and return the [`TempDir`] guard, the
/// stderr output, and the success flag. The [`TempDir`] is yielded
/// first so the caller keeps the project on disk for the duration of
/// its assertions.
pub fn run_project_with_sources(
    package_name: &str,
    perfectionist_dir: &Path,
    shared_target_dir: &Path,
    sources: &[(&str, &str)],
) -> (TempDir, String, bool) {
    let temp = TempDir::new_in(scratch::root()).expect("failed to create temp dir");
    build_project(temp.path(), package_name, perfectionist_dir, sources);
    let (stderr, success) = run_dylint(temp.path(), shared_target_dir);
    (temp, stderr, success)
}

/// Like [`run_project_with_sources`], but appends `dylint_config` to
/// the fixture's `dylint.toml` while staying on plain `--all` (no
/// `--all-targets`, so `#[cfg(test)]` code is excluded). Use it for a
/// rule that is off by default — and so must be enabled through the
/// appended config — but whose fixtures still rely on the non-test
/// build. Pass an empty `dylint_config` for the default configuration.
pub fn run_project_with_sources_and_config(
    package_name: &str,
    perfectionist_dir: &Path,
    shared_target_dir: &Path,
    sources: &[(&str, &str)],
    dylint_config: &str,
) -> (TempDir, String, bool) {
    let temp = TempDir::new_in(scratch::root()).expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        package_name,
        perfectionist_dir,
        sources,
        dylint_config,
    );
    let (stderr, success) = run_dylint(temp.path(), shared_target_dir);
    (temp, stderr, success)
}

/// Like [`run_project_with_sources`], but runs `cargo dylint`
/// with `--all-targets` (so the unit-test target — and thus
/// `cfg(test)` code — is checked) and lets the caller append a
/// per-rule configuration table to the fixture's `dylint.toml`. Pass
/// an empty `dylint_config` for the default configuration.
pub fn run_project_with_config(
    package_name: &str,
    perfectionist_dir: &Path,
    shared_target_dir: &Path,
    sources: &[(&str, &str)],
    dylint_config: &str,
) -> (TempDir, String, bool) {
    let temp = TempDir::new_in(scratch::root()).expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        package_name,
        perfectionist_dir,
        sources,
        dylint_config,
    );
    let (stderr, success) = run_dylint_all_targets(temp.path(), shared_target_dir);
    (temp, stderr, success)
}
