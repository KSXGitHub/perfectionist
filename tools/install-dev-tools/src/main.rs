//! Install `cargo-dylint` and `dylint-link` into a workspace-local
//! directory (`.dev-tools/`), at the version `Cargo.lock` resolves
//! `dylint_linting` to. The justfile prepends `.dev-tools/bin` to
//! `PATH` so every recipe picks up these binaries rather than
//! whatever stale global copies the developer last `cargo install`-ed.
//!
//! The install root is `<repo>/.dev-tools/` rather than somewhere
//! under `target/` so it survives `cargo clean --workspace` and can
//! be cached in CI.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Parser;
use serde::Deserialize;

const DYLINT_LIBRARY_CRATE: &str = "dylint_linting";
const INSTALL_DIR: &str = ".dev-tools";

#[derive(Parser)]
#[clap(about = "Install dylint tooling into <root>/.dev-tools/")]
struct Cli {
    #[clap(help = "The root of the perfectionist repository")]
    root: PathBuf,
}

#[derive(Deserialize)]
struct CargoLock {
    package: Vec<LockedPackage>,
}

#[derive(Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
}

fn main() -> ExitCode {
    let Cli { root } = Cli::parse();
    let version = locked_dylint_version(&root);
    let install_root = root.join(INSTALL_DIR);

    eprintln!(
        "Installing cargo-dylint and dylint-link {version} into {}",
        install_root.display(),
    );

    for crate_name in ["cargo-dylint", "dylint-link"] {
        let status = Command::new("cargo")
            .env("CARGO_INSTALL_ROOT", &install_root)
            .args(["install", "--locked", "--version"])
            .arg(&version)
            .arg(crate_name)
            .status()
            .unwrap_or_else(|error| panic!("spawn `cargo install {crate_name}`: {error}"));
        // `cargo install` exits 0 both on a fresh install and when
        // the requested version is already present, so success here
        // means "the binary is on disk at the right version" in
        // either case. A non-zero exit (typically a different
        // version is already installed) is the developer's signal
        // to delete `.dev-tools/` and rerun.
        if !status.success() {
            eprintln!(
                "`cargo install {crate_name}` failed (exit {status}). \
                 If a different version is already installed under \
                 {}, remove it and rerun.",
                install_root.display(),
            );
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn locked_dylint_version(root: &std::path::Path) -> String {
    let lock_path = root.join("Cargo.lock");
    let text = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", lock_path.display()));
    let lock: CargoLock = toml::from_str(&text)
        .unwrap_or_else(|error| panic!("parse {}: {error}", lock_path.display()));
    lock.package
        .into_iter()
        .find(|pkg| pkg.name == DYLINT_LIBRARY_CRATE)
        .map(|pkg| pkg.version)
        .unwrap_or_else(|| {
            panic!(
                "{DYLINT_LIBRARY_CRATE} not found in {}",
                lock_path.display(),
            )
        })
}
