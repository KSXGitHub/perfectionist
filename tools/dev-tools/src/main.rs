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

use std::env;
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_toml::Manifest;
use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use derive_more::Display;
use pipe_trait::Pipe;

const DYLINT_LIBRARY_CRATE: &str = "dylint_linting";
const INSTALL_DIR: &str = ".dev-tools";

#[derive(Parser)]
#[clap(about = "Manage workspace-local tooling under .dev-tools/")]
struct Cli {
    #[clap(help = "The root of the repository")]
    root: PathBuf,
    #[clap(subcommand)]
    command: Sub,
}

#[derive(Subcommand)]
enum Sub {
    #[clap(about = "Install the development tools")]
    Install,
    #[clap(about = "Print the dylint version")]
    DylintVersion,
    #[clap(about = "Append `version=<dylint version>` to $GITHUB_OUTPUT")]
    GhaDylintVersion,
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
    GhaDylintVersion(GhaDylintVersionError),
    DylintVersion(DylintVersionError),
}

fn run(Cli { root, command }: Cli) -> Result<(), RuntimeError> {
    let version = dylint_version(&root).map_err(RuntimeError::DylintVersion)?;
    match command {
        Sub::DylintVersion => println!("{version}"),
        Sub::Install => install(&root, &version).map_err(RuntimeError::Install)?,
        Sub::GhaDylintVersion => {
            gha_dylint_version(&version).map_err(RuntimeError::GhaDylintVersion)?
        }
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
enum GhaDylintVersionError {
    #[display("$GITHUB_OUTPUT is not set: {_0}")]
    EnvVar(env::VarError),
    #[display("Failed to open $GITHUB_OUTPUT: {_0}")]
    OpenFile(io::Error),
    #[display("Failed to write to $GITHUB_OUTPUT: {_0}")]
    WriteFile(io::Error),
}

fn gha_dylint_version(version: &str) -> Result<(), GhaDylintVersionError> {
    use std::io::Write;
    let path = env::var("GITHUB_OUTPUT").map_err(GhaDylintVersionError::EnvVar)?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(GhaDylintVersionError::OpenFile)?;
    writeln!(file, "version={version}").map_err(GhaDylintVersionError::WriteFile)
}

#[derive(Display)]
enum DylintVersionError {
    #[display("Failed to read Cargo.toml: {_0}")]
    ReadManifest(cargo_toml::Error),
    #[display("{DYLINT_LIBRARY_CRATE} is not a dependency in Cargo.toml")]
    NoData,
}

fn dylint_version(root: &Path) -> Result<String, DylintVersionError> {
    root.join("Cargo.toml")
        .pipe(Manifest::from_path)
        .map_err(DylintVersionError::ReadManifest)?
        .dependencies
        .get(DYLINT_LIBRARY_CRATE)
        .ok_or(DylintVersionError::NoData)?
        .try_req()
        .ok()
        .ok_or(DylintVersionError::NoData)?
        .to_owned()
        .pipe(Ok)
}
