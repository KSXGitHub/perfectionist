//! Shell out to `cargo dylint` and capture its output.
//!
//! Every command here is built by [`cargo_command`], which clears the
//! variables the in-process UI harness leaves set — see
//! [`UI_HARNESS_VARS`].

use command_extra::CommandExtra;
use std::path::Path;
use std::process::Command;

/// The dylint-specific environment variables `dylint_testing` sets in
/// *this* process, and which a spawned `cargo dylint` must not inherit.
///
/// A test binary that holds both a UI run configured through
/// `Test::dylint_toml` and a fixture project run through [`run_dylint`]
/// would otherwise lint the fixture under the UI test's environment:
/// the UI harness reaches its driver by setting process-global
/// variables, and a subprocess inherits them.
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
/// on the first UI test, without ever restoring them — so they are still
/// set long after any lock is released.
const UI_HARNESS_VARS: &[&str] = &["DYLINT_LIBRARY_PATH", "DYLINT_LIBS", "DYLINT_TOML"];

/// A `cargo` command rooted at `project_dir`, pointed at
/// `shared_target_dir`, and with [`UI_HARNESS_VARS`] cleared.
fn cargo_command(project_dir: &Path, shared_target_dir: &Path) -> Command {
    UI_HARNESS_VARS
        .iter()
        .fold(Command::new("cargo"), |mut command, key| {
            command.env_remove(key);
            command
        })
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
        .with_args(match all_targets {
            true => ["--", "--all-targets"].as_slice(),
            false => &[],
        })
        .output()
        .expect("failed to run `cargo dylint`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}

#[cfg(test)]
mod tests;
