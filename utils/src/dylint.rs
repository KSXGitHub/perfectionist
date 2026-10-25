//! Shell out to `cargo dylint` and capture its output.
//!
//! A run that loads no libraries reports no diagnostics and exits
//! zero, which an assertion that a rule *stays silent* cannot tell
//! from the rule standing down. `--fail-on-no-libraries` turns that
//! into a test failure.
//!
//! A `cargo dylint` that predates a flag passed here rejects the
//! whole invocation as an unknown argument, so every run fails for a
//! reason that reads as a bad flag. [`check_dylint_version`] checks
//! the binary on `PATH` against the version this workspace pins.
//!
//! Every command here clears the dylint-specific variables the
//! in-process UI harness leaves set in this process, so a spawned
//! `cargo dylint` never inherits them.

use cargo_toml::{Dependency, Manifest};
use command_extra::CommandExtra;
use derive_more::Display;
use pipe_trait::Pipe;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

mod banner;

/// The dylint-specific environment variables `dylint_testing` sets in
/// *this* process, and which a spawned `cargo dylint` must not inherit.
///
/// A test binary that holds both a UI run built by
/// [`crate::ConfiguredUiTest`] and a fixture project run through
/// [`run_dylint`] would otherwise lint the fixture under the UI
/// test's environment: the UI harness reaches its driver by setting
/// process-global variables, and a subprocess inherits them.
///
/// * `DYLINT_TOML` *replaces* the fixture project's own `dylint.toml`
///   rather than adding to it, so the fixture is linted under the UI
///   fixture's configuration and asserted against the wrong
///   diagnostics.
/// * `DYLINT_LIBRARY_PATH` names directories dylint scans for libraries
///   *in addition to* the ones the fixture's workspace metadata lists.
///   It points at the copy of this crate the UI harness built, so
///   `--all` loads that copy as well as the one it built for the
///   fixture, every lint is registered twice, and rustc aborts the run
///   with `duplicate specification of lint`.
/// * `DYLINT_LIBS` names the library files the driver loads, for the
///   same reason.
///
/// Clearing them is what makes the two kinds of test independent of
/// each other. Serialising them on a mutex does not: `dylint_testing`
/// scopes only `DYLINT_TOML` to one UI run, and sets the other two once,
/// on the first UI test, without ever restoring them. They are therefore
/// still set long after the lock [`crate::ui_test`] takes is released.
const UI_HARNESS_VARS: &[&str] = &["DYLINT_LIBRARY_PATH", "DYLINT_LIBS", "DYLINT_TOML"];

/// A `cargo` command rooted at `project_dir`, pointed at
/// `shared_target_dir`, and with [`UI_HARNESS_VARS`] cleared.
pub(crate) fn cargo_command(project_dir: &Path, shared_target_dir: &Path) -> Command {
    "cargo"
        .pipe(Command::new)
        .without_envs(UI_HARNESS_VARS)
        .with_current_dir(project_dir)
        .with_env("CARGO_TARGET_DIR", shared_target_dir)
}

/// Run `cargo dylint --all` inside `project_dir`, with
/// `CARGO_TARGET_DIR` pointed at `shared_target_dir` so the build
/// artefacts are reused across invocations.
pub fn run_dylint(project_dir: &Path, shared_target_dir: &Path) -> (String, bool) {
    run_dylint_inner(project_dir, shared_target_dir, false)
}

/// Like [`run_dylint`], but forwards `--all-targets` to the underlying
/// `cargo check`. Rules that can only observe test code in a build
/// where `cfg(test)` is active (anything reading `#[cfg(test)]` or
/// `#[test]`) need the unit-test target this flag adds.
pub fn run_dylint_all_targets(project_dir: &Path, shared_target_dir: &Path) -> (String, bool) {
    run_dylint_inner(project_dir, shared_target_dir, true)
}

/// Like [`run_dylint`], but applies the lints' autofixes to the
/// fixture's sources instead of only reporting them, leaving the
/// caller to inspect what the rewrite produced.
///
/// The `cargo fix` flags are not the caller's business: a fixture is a
/// throwaway tree with no version control, which `cargo fix` refuses to
/// rewrite unless told the absence is expected.
pub fn run_dylint_fix(project_dir: &Path, shared_target_dir: &Path) -> (String, bool) {
    let output = cargo_command(project_dir, shared_target_dir)
        .with_arg("dylint")
        .with_arg("--fix")
        .with_arg("--all")
        .with_arg("--fail-on-no-libraries")
        .with_arg("--")
        .with_arg("--lib")
        .with_arg("--allow-no-vcs")
        .with_arg("--allow-dirty")
        .output()
        .expect("failed to run `cargo dylint --fix`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}

fn run_dylint_inner(
    project_dir: &Path,
    shared_target_dir: &Path,
    all_targets: bool,
) -> (String, bool) {
    let output = cargo_command(project_dir, shared_target_dir)
        .with_arg("dylint")
        .with_arg("--all")
        .with_arg("--fail-on-no-libraries")
        .with_args(match all_targets {
            true => ["--", "--all-targets"].as_slice(),
            false => &[],
        })
        .output()
        .expect("failed to run `cargo dylint`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}

/// The dependency whose version requirement the `cargo dylint` on
/// `PATH` has to satisfy.
const DYLINT_LIBRARY_CRATE: &str = "dylint_linting";

/// Installing the pinned version belongs to the recipe this names,
/// never to a test run.
const REMEDY: &str = "run `just install-dev-tools`";

/// What a check of the `cargo dylint` on `PATH` can come out short of.
#[derive(Display)]
pub enum DylintVersionError {
    #[display("Failed to read Cargo.toml: {_0}")]
    ReadManifest(cargo_toml::Error),
    #[display("Cargo.toml pins {DYLINT_LIBRARY_CRATE} to no version requirement")]
    Pin,
    #[display("Failed to spawn `cargo dylint`: {_0}; {REMEDY}")]
    Spawn(io::Error),
    #[display("`cargo dylint --version` failed with {_0}; {REMEDY}\n{_1}")]
    Status(ExitStatus, String),
    #[display("`cargo dylint --version` printed {_0:?} rather than a version banner; {REMEDY}")]
    Unreadable(String),
    #[display("cargo-dylint {_0} is not the pinned {DYLINT_LIBRARY_CRATE} {_1}; {REMEDY}")]
    Mismatch(String, String),
}

/// The version `repository_dir`'s manifest pins
/// [`DYLINT_LIBRARY_CRATE`] to, which `cargo install --version` reads
/// as that one release rather than as a range.
fn pinned_version(repository_dir: &Path) -> Result<String, DylintVersionError> {
    repository_dir
        .join("Cargo.toml")
        .pipe(Manifest::from_path)
        .map_err(DylintVersionError::ReadManifest)?
        .dependencies
        .get(DYLINT_LIBRARY_CRATE)
        .map(Dependency::try_req)
        .and_then(Result::ok)
        .map(str::to_owned)
        .ok_or(DylintVersionError::Pin)
}

/// Check the `cargo dylint` that [`run_dylint`] would spawn against
/// the version `repository_dir`'s manifest pins, returning that
/// version.
pub fn check_dylint_version(repository_dir: &Path) -> Result<String, DylintVersionError> {
    let pinned = pinned_version(repository_dir)?;
    let output = "cargo"
        .pipe(Command::new)
        .with_current_dir(repository_dir)
        .with_arg("dylint")
        .with_arg("--version")
        .output()
        .map_err(DylintVersionError::Spawn)?;
    if !output.status.success() {
        let stderr = output
            .stderr
            .pipe(String::from_utf8)
            .expect("dylint stderr is not UTF-8")
            .trim()
            .to_owned();
        return Err(DylintVersionError::Status(output.status, stderr));
    }
    let reported = output
        .stdout
        .pipe(String::from_utf8)
        .expect("dylint stdout is not UTF-8");
    let Some(installed) = banner::parse_version(&reported) else {
        return Err(DylintVersionError::Unreadable(reported.trim().to_owned()));
    };
    match installed == pinned {
        true => Ok(pinned),
        false => Err(DylintVersionError::Mismatch(installed.to_owned(), pinned)),
    }
}

#[cfg(test)]
mod tests;
