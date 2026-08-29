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
use std::ffi::OsStr;
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
    // The crate root's directory decides first. Cargo names an
    // integration test after its file, so `tests/build_script_env.rs`
    // reaches rustc under a crate name a build script's prefix also
    // matches; the path is the stronger signal, and a build script
    // never roots in one of these directories.
    match root.and_then(target_directory) {
        Some("tests") => CargoTarget::IntegrationTest,
        Some("benches") => CargoTarget::Benchmark,
        Some("examples") => CargoTarget::Example,
        _ if crate_name.starts_with(BUILD_SCRIPT_CRATE_NAME_PREFIX) => CargoTarget::BuildScript,
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
/// `tests/<crate>/src/lib.rs`, or one whose own directory is named
/// `examples`) classified as a library.
fn target_directory(root: &Path) -> Option<&str> {
    let parent = root.parent();
    // A `main.rs` leaf is ambiguous: `tests/main.rs` is the flat form
    // for a target named `main`, while `tests/foo/main.rs` is the
    // subdirectory form for a target named `foo`. Try the grandparent
    // first, because a target directory sits at the package root: in
    // `examples/tests/main.rs` the `tests` component is the target's
    // *name*, and only `examples` is the directory Cargo rooted it in.
    // Trying the parent first would read that as an integration test.
    // The fallback covers `tests/main.rs`, whose grandparent is
    // nothing.
    //
    // A parent of `src` is not a target name but a package root, and
    // rustc is handed a workspace member's root relative to the
    // *workspace*: a member directory named `examples` would otherwise
    // make `examples/src/main.rs` an example, exempting a whole
    // production crate. Cargo does allow a separate target named `src`
    // (`tests/src/main.rs`), which this then reads as a binary — the
    // rarer shape, and it over-lints rather than silently skipping.
    (root.file_name().and_then(|name| name.to_str()) == Some("main.rs"))
        .then(|| parent.filter(|parent| parent.file_name() != Some(OsStr::new("src"))))
        .flatten()
        .and_then(|parent| directory_name(parent.parent()))
        .or_else(|| directory_name(parent))
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
