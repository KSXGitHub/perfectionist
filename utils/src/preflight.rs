//! Verify that the locally-installed `cargo-dylint` and `dylint-link`
//! match the dylint version this workspace pins in `Cargo.lock`.
//!
//! Both binaries are external to the cargo dependency graph but their
//! ABI is coupled to it: `dylint-link` is the linker (see
//! `.cargo/config.toml`) used to produce the perfectionist `cdylib`,
//! and `cargo-dylint` is the loader used by `cli/warmup.rs` and a
//! handful of integration tests. A major-version mismatch produces an
//! opaque error far from its cause; running this check first surfaces
//! the cause and points at `just install-dev-tools` as the remedy.
//!
//! The required version is read from `Cargo.lock`'s `dylint_linting`
//! entry rather than being hardcoded so that bumping the dependency
//! in `Cargo.toml` is the single edit needed to bump the required
//! tool version.
//!
//! The check compares *major* versions only: dylint follows SemVer
//! for its tool/library ABI, so minor or patch drift is compatible.
//!
//! `cargo-dylint` exposes its version via `cargo dylint --version`,
//! but `dylint-link --version` forwards to the underlying C linker,
//! so its version is read from `$CARGO_HOME/.crates.toml` instead.
//! Binaries on PATH that aren't recorded there (e.g. distro
//! packages, or prebuilt downloads from `taiki-e/install-action`)
//! are accepted without a version check — better to wave through an
//! unknowable version than to refuse a valid CI install.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde::Deserialize;

const REQUIRED_PACKAGE: &str = "dylint_linting";

#[derive(Deserialize)]
struct CargoLock {
    package: Vec<LockedPackage>,
}

#[derive(Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
}

/// Resolved version of `dylint_linting` from `Cargo.lock` at
/// `perfectionist_dir`. Panics if the entry is missing — that means
/// either the file is corrupt or the dep was removed, both of which
/// the developer should hear about immediately.
pub fn required_dylint_version(perfectionist_dir: &Path) -> String {
    let lock_path = perfectionist_dir.join("Cargo.lock");
    let text = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", lock_path.display()));
    let lock: CargoLock = toml::from_str(&text)
        .unwrap_or_else(|error| panic!("parse {}: {error}", lock_path.display()));
    lock.package
        .into_iter()
        .find(|pkg| pkg.name == REQUIRED_PACKAGE)
        .map(|pkg| pkg.version)
        .unwrap_or_else(|| {
            panic!(
                "{REQUIRED_PACKAGE} not found in {}; \
                 has the workspace's dependency on {REQUIRED_PACKAGE} been removed?",
                lock_path.display(),
            )
        })
}

/// Verify both `cargo-dylint` and `dylint-link` are installed at the
/// same major version as `Cargo.lock`'s `dylint_linting`. Returns a
/// multi-line, user-facing remediation message on mismatch.
pub fn check_dylint_tools(perfectionist_dir: &Path) -> Result<(), String> {
    let required = required_dylint_version(perfectionist_dir);
    let required_major = major(&required).ok_or_else(|| {
        format!("malformed {REQUIRED_PACKAGE} version in Cargo.lock: {required:?}")
    })?;

    let mut problems = Vec::new();
    inspect(
        "cargo dylint",
        cargo_dylint_version(),
        required_major,
        &mut problems,
    );
    inspect(
        "dylint-link",
        dylint_link_version(),
        required_major,
        &mut problems,
    );
    if problems.is_empty() {
        return Ok(());
    }
    Err(format!(
        "The locally-installed dylint toolchain doesn't match this workspace's \
         resolved `{REQUIRED_PACKAGE} = {required}`:\n\
         {issues}\n\
         \n\
         Install the matching versions with:\n\
         \n    just install-dev-tools\n",
        issues = problems.join("\n"),
    ))
}

/// Same as [`check_dylint_tools`] but caches the result so repeated
/// calls (e.g. from each `#[test]` in `tests/flat_module_pattern.rs`)
/// don't re-spawn the version probes.
pub fn check_dylint_tools_once(perfectionist_dir: &Path) -> Result<(), &'static str> {
    static CACHED: OnceLock<Result<(), String>> = OnceLock::new();
    match CACHED.get_or_init(|| check_dylint_tools(perfectionist_dir)) {
        Ok(()) => Ok(()),
        Err(message) => Err(message.as_str()),
    }
}

/// Result of probing one of the two dylint binaries.
enum Detection {
    /// Binary is installed and its version was determined.
    Known(String),
    /// Binary is on PATH but we can't tell its version — pass without
    /// a mismatch error.
    Unknown,
    /// Binary is not installed at all.
    Missing,
}

fn inspect(display: &str, detection: Detection, required_major: &str, problems: &mut Vec<String>) {
    match detection {
        Detection::Missing => {
            problems.push(format!("  - `{display}` is not installed"));
        }
        Detection::Unknown => {}
        Detection::Known(found) => {
            let found_major = major(&found).unwrap_or("?");
            if found_major != required_major {
                problems.push(format!(
                    "  - `{display}` is at version {found}, but this workspace needs {required_major}.x",
                ));
            }
        }
    }
}

/// Probe `cargo dylint --version`, which prints `cargo-dylint X.Y.Z`
/// on stdout when installed and exits non-zero with
/// `no such command: \`dylint\`` on stderr when not.
fn cargo_dylint_version() -> Detection {
    let output = match Command::new("cargo").args(["dylint", "--version"]).output() {
        Ok(out) => out,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Detection::Missing,
        Err(_) => return Detection::Unknown,
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("no such command") {
        return Detection::Missing;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    match stdout.split_whitespace().find(|t| looks_like_semver(t)) {
        Some(version) => Detection::Known(version.to_owned()),
        None => Detection::Unknown,
    }
}

/// Read `dylint-link`'s version from `$CARGO_HOME/.crates.toml` (the
/// registry of `cargo install`-managed binaries). Falls back to
/// `Detection::Unknown` when the binary is on PATH but not recorded
/// there — that happens with prebuilt-binary installers used in CI.
fn dylint_link_version() -> Detection {
    let recorded = cargo_home().and_then(|home| crates_toml_version(&home, "dylint-link"));
    if let Some(version) = recorded {
        return Detection::Known(version);
    }
    if binary_on_path("dylint-link") {
        Detection::Unknown
    } else {
        Detection::Missing
    }
}

fn cargo_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("CARGO_HOME") {
        return Some(PathBuf::from(home));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo"))
}

/// Read the installed version of `crate_name` from
/// `$CARGO_HOME/.crates.toml`. Entries in the `[v1]` table look like
///
/// ```text
/// "dylint-link 6.0.0 (registry+https://github.com/rust-lang/crates.io-index)" = ["dylint-link"]
/// ```
///
/// Parsed with a small hand-rolled scanner rather than the `toml`
/// crate because newer `toml` releases reject this file as malformed
/// under TOML 1.1's stricter rules.
fn crates_toml_version(cargo_home: &Path, crate_name: &str) -> Option<String> {
    let text = std::fs::read_to_string(cargo_home.join(".crates.toml")).ok()?;
    let mut in_v1 = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[v1]" {
            in_v1 = true;
            continue;
        }
        if line.starts_with('[') {
            in_v1 = false;
            continue;
        }
        if !in_v1 {
            continue;
        }
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        let key = &rest[..end];
        let mut parts = key.splitn(3, ' ');
        let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
            continue;
        };
        if name == crate_name {
            return Some(version.to_owned());
        }
    }
    None
}

fn binary_on_path(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(binary).is_file())
}

fn looks_like_semver(token: &str) -> bool {
    let mut parts = token.splitn(3, '.');
    let major = parts.next();
    let minor = parts.next();
    let patch = parts.next();
    let is_numeric = |part: Option<&str>| matches!(part, Some(p) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    is_numeric(major) && is_numeric(minor) && patch.is_some_and(|p| !p.is_empty())
}

fn major(version: &str) -> Option<&str> {
    let m = version.split('.').next()?;
    if !m.is_empty() && m.chars().all(|c| c.is_ascii_digit()) {
        Some(m)
    } else {
        None
    }
}
