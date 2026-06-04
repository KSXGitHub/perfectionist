//! Read the `[package].version` field from a Cargo manifest, and
//! the `perfectionist` package's `version` field from a Cargo
//! lockfile.

use super::PACKAGE_NAME;
use super::error::RuntimeError;
use pipe_trait::Pipe;
use serde::Deserialize;

#[derive(Deserialize)]
struct CargoTomlFile {
    package: CargoTomlPackage,
}

#[derive(Deserialize)]
struct CargoTomlPackage {
    version: String,
}

pub(crate) fn parse_cargo_toml_version(toml: &str) -> Result<String, RuntimeError> {
    toml.pipe(toml::from_str::<CargoTomlFile>)
        .map_err(RuntimeError::ParseCargoToml)?
        .package
        .version
        .pipe(Ok)
}

#[derive(Deserialize)]
struct CargoLockFile {
    #[serde(default)]
    package: Vec<CargoLockPackage>,
}

#[derive(Deserialize)]
struct CargoLockPackage {
    name: String,
    version: String,
}

pub(crate) fn parse_cargo_lock_version(toml: &str) -> Result<String, RuntimeError> {
    let lock: CargoLockFile = toml
        .pipe(toml::from_str)
        .map_err(RuntimeError::ParseCargoLock)?;
    let mut matches = lock
        .package
        .into_iter()
        .filter(|pkg| pkg.name == PACKAGE_NAME);
    let first = matches.next().ok_or(RuntimeError::NoLockPackageEntry)?;
    if matches.next().is_some() {
        return Err(RuntimeError::DuplicateLockPackageEntry);
    }
    Ok(first.version)
}
