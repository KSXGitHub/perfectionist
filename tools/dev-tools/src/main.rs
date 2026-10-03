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

use cargo_toml::Manifest;
use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use derive_more::Display;
use pipe_trait::Pipe;
use std::fs::{OpenOptions, read_dir, remove_file};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, io};

const DYLINT_LIBRARY_CRATE: &str = "dylint_linting";
const INSTALL_DIR: &str = ".dev-tools";

/// The crates [`install`] pins, named once so its `cargo install`
/// arguments and the index eviction beside them cannot drift apart.
const PINNED_CRATES: [&str; 2] = ["cargo-dylint", "dylint-link"];

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

/// Where cargo keeps its registry caches: `$CARGO_HOME` when set,
/// otherwise `.cargo` under the platform's home directory.
fn cargo_home() -> Option<PathBuf> {
    match env::var_os("CARGO_HOME") {
        Some(path) => Some(path.into()),
        None => env::home_dir().map(|home| home.join(".cargo")),
    }
}

/// The directory cargo files a crate's index metadata under, following
/// the registry index layout: one directory naming the length for a
/// name shorter than four characters, two character-pair directories
/// for anything longer. `crate_name` is expected lowercase ASCII, as
/// the index spells it.
fn index_prefix(crate_name: &str) -> PathBuf {
    match crate_name.len() {
        1 => PathBuf::from("1"),
        2 => PathBuf::from("2"),
        3 => Path::new("3").join(&crate_name[..1]),
        _ => Path::new(&crate_name[..2]).join(&crate_name[2..4]),
    }
}

/// Drop one crate's cached index metadata under `registry`, silently
/// where there is none to drop.
fn remove_cached_index_entry(registry: &Path, crate_name: &str) {
    let path = registry
        .join(".cache")
        .join(index_prefix(crate_name))
        .join(crate_name);
    if let Err(error) = remove_file(&path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!("warning: failed to remove {}: {error}", path.display());
    }
}

/// Drop cargo's cached registry metadata for [`PINNED_CRATES`].
///
/// [`install`]'s `cargo install --version` is the only step in this
/// workspace that reads the registry index; every other one replays
/// `Cargo.lock` against sources it already has. So a CI cache can
/// carry an index snapshot older than the pinned dylint release with
/// nothing noticing until the pin moves, and then cargo reports a
/// published version as `could not find <crate> in registry`. Cargo
/// refreshes a cached entry with a conditional request and keeps what
/// it holds when the answer is "unchanged", so removing the entry is
/// what forces the version to be fetched outright.
///
/// Best effort throughout: what is already absent passes in silence,
/// and what cannot be read or removed is warned about and skipped
/// rather than failing the install.
fn evict_cached_index_entries() {
    let Some(home) = cargo_home() else {
        eprintln!("warning: cannot locate CARGO_HOME; not refreshing index metadata");
        return;
    };
    let index = home.join("registry").join("index");
    let registries = match read_dir(&index) {
        Ok(registries) => registries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(error) => {
            eprintln!("warning: failed to read {}: {error}", index.display());
            return;
        }
    };
    for registry in registries.flatten() {
        let registry_dir = registry.path();
        for crate_name in PINNED_CRATES {
            remove_cached_index_entry(&registry_dir, crate_name);
        }
    }
}

fn install(root: &Path, version: &str) -> Result<(), InstallError> {
    let install_root = root.join(INSTALL_DIR);

    eprintln!(
        "Installing cargo-dylint and dylint-link {version} into {}",
        install_root.display(),
    );

    evict_cached_index_entries();

    "cargo"
        .pipe(Command::new)
        .with_env("CARGO_INSTALL_ROOT", &install_root)
        .with_arg("install")
        .with_arg("--locked")
        .with_arg("--version")
        .with_arg(version)
        .with_args(PINNED_CRATES)
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
