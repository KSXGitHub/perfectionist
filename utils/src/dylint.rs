//! Shell out to `cargo dylint --all` and capture its output.

use command_extra::CommandExtra;
use pipe_trait::Pipe;
use std::path::Path;
use std::process::Command;

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
    let output = "cargo"
        .pipe(Command::new)
        .with_arg("dylint")
        .with_arg("--all")
        .with_current_dir(project_dir)
        .with_env("CARGO_TARGET_DIR", shared_target_dir)
        .with_args(match all_targets {
            true => ["--", "--all-targets"].as_slice(),
            false => &[],
        })
        .output()
        .expect("failed to run `cargo dylint`");
    let stderr = String::from_utf8(output.stderr).expect("dylint stderr is not UTF-8");
    (stderr, output.status.success())
}
