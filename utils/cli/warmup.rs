//! Pre-warm the shared integration-test target dir.
//!
//! Materialises a minimal canonical fixture and runs
//! `cargo dylint --all` against it with `CARGO_TARGET_DIR` pointed at
//! `<perfectionist_dir>/target/integration-fixtures`. After this
//! runs once, subsequent integration tests reuse the compiled std and
//! the built perfectionist plugin instead of paying the cost from
//! cold.
//!
//! Invoked by the `warmup-integration-tests` recipe in the justfile.
//! Takes the perfectionist crate directory as a single argument; the
//! shared target dir is computed relative to it.

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[clap(about = "Pre-warm the shared integration-test target dir")]
struct WarmupCli {
    #[clap(help = "The root of the repository")]
    root: PathBuf,
}

fn main() -> ExitCode {
    let WarmupCli { root } = WarmupCli::parse();
    // Canonicalise so the perfectionist path embedded in the fixture's
    // `dylint.toml` resolves regardless of where the fixture sits.
    let root = std::fs::canonicalize(&root)
        .unwrap_or_else(|error| panic!("canonicalise {}: {error}", root.display()));
    let shared_target_dir = root.join("target").join("integration-fixtures");
    let warmup_project_dir = root.join("target").join("integration-fixtures-warmup");

    // Start the fixture fresh so the warmup is reproducible. A
    // missing dir is fine — that's the expected state on a clean
    // checkout — but any other I/O error would leave stale files
    // around, so fail loudly.
    match std::fs::remove_dir_all(&warmup_project_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!(
            "failed to clean up {}: {error}",
            warmup_project_dir.display(),
        ),
    }
    std::fs::create_dir_all(&warmup_project_dir).expect("create new warmup project dir");

    _utils::build_project(
        &warmup_project_dir,
        "fixture_warmup",
        &root,
        &[("src/lib.rs", "")],
    );

    let (stderr, success) = _utils::run_dylint(&warmup_project_dir, &shared_target_dir);
    if !success {
        eprintln!("{stderr}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
