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
use std::{path::PathBuf, process};

#[derive(Parser)]
#[clap(about = "Pre-warm the shared integration-test target dir")]
struct WarmupCli {
    #[clap(help = "The root of the repository")]
    root: PathBuf,
}

fn main() {
    let WarmupCli { root } = WarmupCli::parse();
    let shared_target_dir = root.join("target").join("integration-fixtures");
    let warmup_project_dir = root.join("target").join("integration-fixtures-warmup");

    // Start the fixture fresh so the warmup is reproducible.
    std::fs::remove_dir_all(&warmup_project_dir).expect("delete old warmup project dir");
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
        process::exit(1);
    }
}
