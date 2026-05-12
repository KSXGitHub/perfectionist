//! Manage `cargo-dylint` and `dylint-link` under a workspace-local
//! directory (`.dev-tools/`), at the version `Cargo.lock` resolves
//! `dylint_linting` to. The justfile prepends `.dev-tools/bin` to
//! `PATH` so every recipe picks up these binaries rather than
//! whatever stale global copies the developer last `cargo install`-ed.
//!
//! The install root is `<repo>/.dev-tools/` rather than somewhere
//! under `target/` so it survives `cargo clean --workspace` and can
//! be cached in CI.
//!
//! The justfile invokes this binary with
//! `cargo --config 'target."cfg(all())".linker="cc"'` so a fresh
//! checkout — where `dylint-link` (the workspace's linker per
//! `.cargo/config.toml`) is not yet on PATH — can still compile it.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use pipe_trait::Pipe;
use serde::Deserialize;

const DYLINT_LIBRARY_CRATE: &str = "dylint_linting";
const INSTALL_DIR: &str = ".dev-tools";

#[derive(Parser)]
#[clap(about = "Manage workspace-local dylint tooling under .dev-tools/")]
struct Cli {
    #[clap(help = "The root of the perfectionist repository")]
    root: PathBuf,

    #[clap(subcommand)]
    command: Subcmd,
}

#[derive(Subcommand)]
enum Subcmd {
    /// Install cargo-dylint and dylint-link into `<root>/.dev-tools/`
    /// at the version pinned in `Cargo.lock`.
    Install,
    /// Print the pinned dylint version (read from `Cargo.lock`) and
    /// exit. Used by CI to derive the `.dev-tools/` cache key.
    PrintVersion,
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
    let Cli { root, command } = Cli::parse();
    let version = locked_dylint_version(&root);
    match command {
        Subcmd::PrintVersion => {
            println!("{version}");
            ExitCode::SUCCESS
        }
        Subcmd::Install => install(&root, &version),
    }
}

fn install(root: &Path, version: &str) -> ExitCode {
    let install_root = root.join(INSTALL_DIR);

    eprintln!(
        "Installing cargo-dylint and dylint-link {version} into {}",
        install_root.display(),
    );

    let status = "cargo"
        .pipe(Command::new)
        .with_env("CARGO_INSTALL_ROOT", &install_root)
        .with_arg("install")
        .with_arg("--locked")
        .with_arg("--version")
        .with_arg(version)
        .with_arg("cargo-dylint")
        .with_arg("dylint-link")
        .status()
        .expect("spawn `cargo install`");
    if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn locked_dylint_version(root: &Path) -> String {
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
