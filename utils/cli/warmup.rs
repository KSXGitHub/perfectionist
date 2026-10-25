//! Pre-warm the target dirs the integration tests share.
//!
//! Materialises a minimal canonical fixture and runs
//! `cargo dylint --all` against it with `CARGO_TARGET_DIR` pointed at
//! `<perfectionist_dir>/target/integration-fixtures`. After this
//! runs once, subsequent integration tests reuse the compiled std and
//! the built perfectionist plugin instead of paying the cost from
//! cold.
//!
//! The canonical fixture declares the registry crates the fixture
//! projects depend on, so the one-time cost of compiling them is paid
//! here rather than inside whichever test reaches one first.
//!
//! Each codebase fixture under `tests/fixtures/autofix/` then gets
//! the same treatment in a target dir of its own, since what its
//! cache has to hold is decided by its dependency list.
//!
//! The run resolves `cargo dylint` from `PATH`, so it starts by
//! checking that binary against the version this workspace pins.
//!
//! Invoked by the `warmup-integration-tests` recipe in the justfile.
//! Takes the perfectionist crate directory as a single argument; the
//! shared target dir is computed relative to it.

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

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
    let version = match _utils::check_dylint_version(&root) {
        Ok(version) => version,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("warmup: cargo-dylint {version}");

    let shared_target_dir = root.join("target").join("integration-fixtures");
    let warmup_project_dir = root.join("target").join("integration-fixtures-warmup");

    // Start the fixture fresh so the warmup is reproducible. A
    // missing dir is fine, being the expected state on a clean
    // checkout. Any other I/O error would leave stale files around,
    // so fail loudly.
    match std::fs::remove_dir_all(&warmup_project_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!(
            "failed to clean up {}: {error}",
            warmup_project_dir.display(),
        ),
    }
    std::fs::create_dir_all(&warmup_project_dir).expect("create new warmup project dir");

    // The sources overwrite the generated manifest, which carries no
    // dependencies. `derive_more` is taken with the feature set the
    // widest fixture asks for, since a narrower one would be a
    // different unit and leave that fixture paying for its own.
    let manifest = format!(
        "{}\n[dependencies]\n\
         command-extra = \"1.2.0\"\n\
         derive_more = {{ version = \"2\", features = [\"full\"] }}\n",
        _utils::fixture_cargo_toml("fixture_warmup"),
    );
    _utils::build_project(
        &warmup_project_dir,
        "fixture_warmup",
        &root,
        &[("Cargo.toml", &manifest), ("src/lib.rs", "")],
    );

    let started = Instant::now();
    let (stderr, success) = _utils::run_dylint(&warmup_project_dir, &shared_target_dir);
    let elapsed = started.elapsed();
    if !success {
        eprintln!("{stderr}");
        return ExitCode::FAILURE;
    }
    eprintln!("warmup: cargo dylint --all took {elapsed:.2?}");

    for codebase in _utils::autofix::codebases(&root) {
        let started = Instant::now();
        _utils::autofix::warm(&root, &codebase);
        eprintln!("warmup: {codebase} took {:.2?}", started.elapsed());
    }

    ExitCode::SUCCESS
}
