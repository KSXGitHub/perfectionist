//! The cargo invocations a round is made of, plus the ones that
//! bracket a run.
//!
//! A round is a perfectionist autofix, a Clippy autofix, and a
//! rustfmt pass, in that order. Each is one invocation over the whole
//! fixture workspace rather than one per member: `cargo fix` re-runs
//! every member whatever the cache holds, so the per-invocation floor
//! is paid once instead of once per crate.
//!
//! Every invocation shares one `CARGO_TARGET_DIR` with the rest of
//! its fixture, so a round reuses the compiled standard library, the
//! registry dependencies, and the perfectionist plugin the warmup
//! built.
//!
//! The fixes are told to rewrite a tree that has no version control
//! and is already dirty, which a fixture materialised in a temp dir
//! always is, and which `cargo fix` otherwise refuses.

use crate::dylint::cargo_command;
use command_extra::CommandExtra;
use std::path::Path;
use std::process::Command;

/// What one invocation left behind. `stdout` carries the JSON a check
/// is asked for and the output a fixture's `main` printed; `stderr`
/// carries the rendered diagnostics a failure report quotes.
pub struct Outcome {
    /// How the invocation was spelled, for a failure report to name.
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// The flags `cargo fix` needs to rewrite the project at all.
const FIX_PERMISSION: &[&str] = &["--allow-no-vcs", "--allow-dirty"];

/// The whole project, test targets included, which is what every
/// check and fix here covers. A rule that can only see test code in a
/// build where `cfg(test)` is active needs the unit-test target, and a
/// project's own behaviour assertions live in one.
const WHOLE_WORKSPACE: &[&str] = &["--workspace", "--all-targets"];

/// Ask for machine-readable diagnostics, which arrive on stdout.
const JSON: &str = "--message-format=json";

/// Apply perfectionist's own autofixes over the whole project.
pub fn dylint_fix(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir)
            .with_args(["dylint", "--fix", "--all", "--fail-on-no-libraries", "--"])
            .with_args(WHOLE_WORKSPACE)
            .with_args(FIX_PERMISSION),
        "cargo dylint --fix",
    )
}

/// Report what perfectionist still finds, without rewriting anything.
pub fn dylint_check(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir)
            .with_args(["dylint", "--all", "--fail-on-no-libraries", "--"])
            .with_args(WHOLE_WORKSPACE)
            .with_arg(JSON),
        "cargo dylint",
    )
}

/// Apply Clippy's autofixes over the whole project.
pub fn clippy_fix(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir)
            .with_args(["clippy", "--fix"])
            .with_args(WHOLE_WORKSPACE)
            .with_args(FIX_PERMISSION),
        "cargo clippy --fix",
    )
}

/// Report what Clippy still finds, without rewriting anything.
pub fn clippy_check(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir)
            .with_arg("clippy")
            .with_args(WHOLE_WORKSPACE)
            .with_arg(JSON),
        "cargo clippy",
    )
}

/// Reformat the whole project, which closes a round.
pub fn rustfmt(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir).with_args(["fmt", "--all"]),
        "cargo fmt --all",
    )
}

/// Compile the project's own tests without running them, which is
/// what a warmup needs of the lane plain rustc builds.
pub fn build_tests(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir).with_args(["test", "--workspace", "--no-run"]),
        "cargo test --workspace --no-run",
    )
}

/// Run the project's own tests, which carry its behaviour assertions.
pub fn test(project_dir: &Path, target_dir: &Path) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir).with_args(["test", "--workspace"]),
        "cargo test --workspace",
    )
}

/// Run the project's `main`, whose stdout is the behaviour a round
/// must leave alone.
pub fn run_cli(project_dir: &Path, target_dir: &Path, package: &str) -> Outcome {
    collect(
        cargo_command(project_dir, target_dir).with_args(["run", "--quiet", "--package", package]),
        &format!("cargo run --package {package}"),
    )
}

/// Spawn a prepared command and collect what it left behind.
/// `command` is how the invocation is spelled, for a report to name.
fn collect(mut prepared: Command, command: &str) -> Outcome {
    let output = prepared
        .output()
        .unwrap_or_else(|error| panic!("failed to run `{command}`: {error}"));
    Outcome {
        command: command.to_owned(),
        stdout: String::from_utf8(output.stdout).unwrap_or_else(|error| {
            panic!("`{command}` printed stdout that is not UTF-8: {error}")
        }),
        stderr: String::from_utf8(output.stderr).unwrap_or_else(|error| {
            panic!("`{command}` printed stderr that is not UTF-8: {error}")
        }),
        success: output.status.success(),
    }
}
