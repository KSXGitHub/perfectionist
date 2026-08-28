//! Classifying which Cargo target the crate under compilation is.
//!
//! Cargo compiles a package's integration tests, benchmarks, examples,
//! and build script as their own crates, so a `--all-targets` run hands
//! each of them to every lint pass separately. A rule whose scope is
//! "production code" needs to tell those apart from the library or
//! binary, and the only evidence available inside the compiler is the
//! crate name Cargo passed and where the crate root sits on disk.

use rustc_hir::def_id::LOCAL_CRATE;
use rustc_lint::{LateContext, LintContext};
use std::path::Path;

/// Cargo's crate-name prefix for a build script. Cargo names the
/// build-script target `build-script-<file stem>`, which reaches rustc
/// as `--crate-name build_script_<file stem>` — `build_script_build`
/// for the default `build.rs`, `build_script_mk` for a `build = "mk.rs"`
/// package. Keying off the prefix rather than the file name is what
/// covers the renamed case.
///
/// This is a convention, not a stability guarantee: Cargo could in
/// principle name build-script crates something else, and a package
/// could in principle publish a library actually called
/// `build_script_*`. Both are remote enough that the prefix is the
/// signal every tool in the ecosystem uses.
const BUILD_SCRIPT_CRATE_NAME_PREFIX: &str = "build_script_";

/// Which Cargo target the crate under compilation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CargoTarget {
    /// An integration test — a crate rooted in `tests/`.
    IntegrationTest,
    /// A benchmark — a crate rooted in `benches/`.
    Benchmark,
    /// An example — a crate rooted in `examples/`.
    Example,
    /// A build script — `build.rs`, or whatever `Cargo.toml`'s `build`
    /// key names.
    BuildScript,
    /// The package's library or one of its binaries: the code the
    /// package exists to ship.
    LibOrBin,
}

impl CargoTarget {
    /// Whether the whole crate is test code. An integration test and a
    /// benchmark are compiled under `cfg(test)` and exist only to
    /// exercise the library, so every item in them is test code —
    /// unlike a library's own unit-test build, where `#[cfg(test)]` is
    /// what separates the test items from the production ones.
    ///
    /// An example is *not* test code by this measure even when
    /// `test = true` makes Cargo compile it under `cfg(test)`: it is
    /// documentation that readers copy, and holding it to the same
    /// standard as the library is the point of it.
    pub(crate) fn is_test_target(self) -> bool {
        matches!(self, CargoTarget::IntegrationTest | CargoTarget::Benchmark)
    }

    /// Whether the crate is one of Cargo's separate non-library
    /// targets — an integration test, benchmark, or example.
    pub(crate) fn is_separate_target(self) -> bool {
        matches!(
            self,
            CargoTarget::IntegrationTest | CargoTarget::Benchmark | CargoTarget::Example,
        )
    }
}

/// Classify the crate currently being compiled.
pub(crate) fn crate_target(cx: &LateContext<'_>) -> CargoTarget {
    let crate_name = cx.tcx.crate_name(LOCAL_CRATE);
    let root = cx.sess().local_crate_source_file();
    let root = root.as_ref().and_then(|root| root.local_path());
    classify(crate_name.as_str(), root)
}

/// The classification behind [`crate_target`], split out as a pure
/// function so the path arithmetic can be unit-tested without a
/// compiler context.
fn classify(crate_name: &str, root: Option<&Path>) -> CargoTarget {
    if crate_name.starts_with(BUILD_SCRIPT_CRATE_NAME_PREFIX) {
        return CargoTarget::BuildScript;
    }
    match root.and_then(target_directory) {
        Some("tests") => CargoTarget::IntegrationTest,
        Some("benches") => CargoTarget::Benchmark,
        Some("examples") => CargoTarget::Example,
        _ => CargoTarget::LibOrBin,
    }
}

/// The name of the Cargo target directory `root` is rooted in, when it
/// is one of the separate-target directories.
///
/// Cargo roots these targets at `<dir>/<name>.rs` or
/// `<dir>/<name>/main.rs`, where `<dir>` is `tests/`, `benches/`, or
/// `examples/`, while a library or binary roots under `src/`
/// (`lib.rs`, `main.rs`, `bin/<name>.rs`). Matching the target
/// directory itself — not some farther ancestor — keeps a library that
/// merely lives below such a directory (a workspace member at
/// `tests/<crate>/src/lib.rs`, say) classified as a library.
fn target_directory(root: &Path) -> Option<&str> {
    let parent = root.parent();
    // Flat form `<dir>/<name>.rs` — including `<dir>/main.rs`, a target
    // literally named `main` — roots directly in the target directory,
    // so check the immediate parent first. The subdirectory form
    // `<dir>/<name>/main.rs` roots one level deeper, so for a `main.rs`
    // leaf also accept the grandparent. Checking the parent first is
    // what keeps `tests/main.rs` matched instead of walking past `tests`
    // to nothing.
    directory_name(parent).or_else(|| {
        (root.file_name().and_then(|name| name.to_str()) == Some("main.rs"))
            .then(|| directory_name(parent.and_then(Path::parent)))
            .flatten()
    })
}

/// `dir`'s final component, when it is one of Cargo's separate-target
/// directories.
fn directory_name(dir: Option<&Path>) -> Option<&str> {
    dir.and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .filter(|name| matches!(*name, "tests" | "benches" | "examples"))
}

#[cfg(test)]
mod tests;
