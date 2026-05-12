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

use std::fs::read_to_string;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use derive_more::Display;
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
    command: Sub,
}

#[derive(Subcommand)]
enum Sub {
    #[clap(about = "Install the development tools")]
    Install,
    #[clap(about = "Print the dylint version")]
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
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Display)]
enum RuntimeError {
    Install(InstallError),
    DylintVersion(DylintVersionError),
}

fn run(Cli { root, command }: Cli) -> Result<(), RuntimeError> {
    let version = dylint_version(&root).map_err(RuntimeError::DylintVersion)?;
    match command {
        Sub::PrintVersion => println!("{version}"),
        Sub::Install => install(&root, &version).map_err(RuntimeError::Install)?,
    }
    Ok(())
}

#[derive(Display)]
enum InstallError {
    #[display("Failed to spawn `cargo install`: {_0}")]
    Spawn(io::Error),
    #[display("Process exits with an error")]
    Status,
}

fn install(root: &Path, version: &str) -> Result<(), InstallError> {
    let install_root = root.join(INSTALL_DIR);

    eprintln!(
        "Installing cargo-dylint and dylint-link {version} into {}",
        install_root.display(),
    );

    "cargo"
        .pipe(Command::new)
        .with_env("CARGO_INSTALL_ROOT", &install_root)
        .with_arg("install")
        .with_arg("--locked")
        .with_arg("--version")
        .with_arg(version)
        .with_arg("cargo-dylint")
        .with_arg("dylint-link")
        .status()
        .map_err(InstallError::Spawn)?
        .success()
        .then_some(())
        .ok_or(InstallError::Status)
}

#[derive(Display)]
enum DylintVersionError {
    #[display("Failed to read Cargo.lock: {_0}")]
    ReadLockFile(io::Error),
    #[display("Failed to parse Cargo.lock: {_0}")]
    ParseLockFile(toml::de::Error),
    #[display("{DYLINT_LIBRARY_CRATE} not found in Cargo.lock")]
    NoData,
}

fn dylint_version(root: &Path) -> Result<String, DylintVersionError> {
    root.join("Cargo.lock")
        .pipe(read_to_string)
        .map_err(DylintVersionError::ReadLockFile)?
        .pipe_as_ref(toml::from_str::<CargoLock>)
        .map_err(DylintVersionError::ParseLockFile)?
        .package
        .into_iter()
        .find(|pkg| pkg.name == DYLINT_LIBRARY_CRATE)
        .map(|pkg| pkg.version)
        .ok_or(DylintVersionError::NoData)
}
