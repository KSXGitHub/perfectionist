//! Shell out to `cargo dylint --all` and capture its output.

use std::{path::Path, process::Command};

use command_extra::CommandExtra;
use pipe_trait::Pipe;

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

fn run_dylint_inner(
    project_dir: &Path,
    shared_target_dir: &Path,
    all_targets: bool,
) -> (String, bool) {
    let mut command = "cargo"
        .pipe(Command::new)
        .with_arg("dylint")
        .with_arg("--all")
        .with_current_dir(project_dir)
        .with_env("CARGO_TARGET_DIR", shared_target_dir);
    if all_targets {
        command = command.with_arg("--").with_arg("--all-targets");
    }
    let output = command.output().expect("failed to run `cargo dylint`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}
