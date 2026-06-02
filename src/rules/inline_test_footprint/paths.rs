//! Separate-target detection and the extraction-target path arithmetic
//! the inline-style help text uses.

use std::path::{Path, PathBuf};

use rustc_lint::{LateContext, LintContext};
use rustc_span::{FileName, SourceFile};

/// Whether the crate currently being compiled is a *separate*
/// non-library target — an integration test (`tests/`), benchmark
/// (`benches/`), or example (`examples/`) crate — rather than the
/// library or binary whose unit-test footprint this rule governs.
///
/// Cargo compiles each of those as its own crate, so a `--all-targets`
/// run hands them to the rule too. For the ones built under `cfg(test)`
/// — integration tests, benchmarks, and `test = true` examples — the
/// top-level `#[test]` functions *are the target*, not unit tests
/// misplaced inside a production file; sitting beside ordinary helper
/// `fn`s the rule counts as production, they would otherwise be
/// flagged. (A plain example builds without `cfg(test)`, so the rule
/// sees no test items there regardless, but it is skipped for the same
/// reason and to cover the `test = true` case.) The rule only governs
/// where a library or binary keeps its unit tests, so skip these
/// targets whole.
///
/// Detection keys off the crate root's directory: Cargo roots these
/// targets at `<dir>/<name>.rs` or `<dir>/<name>/main.rs`, where `<dir>`
/// is `tests/`, `benches/`, or `examples/`, while a library/binary roots
/// under `src/` (`lib.rs`, `main.rs`, `bin/<name>.rs`). Matching the
/// target directory itself — not some farther ancestor — keeps a
/// library that merely lives below such a directory (a workspace member
/// at `tests/<crate>/src/lib.rs`, say) in scope.
pub(super) fn is_separate_test_target(cx: &LateContext<'_>) -> bool {
    let Some(root) = cx.sess().local_crate_source_file() else {
        return false;
    };
    let Some(path) = root.local_path() else {
        return false;
    };
    is_separate_target_path(path)
}

/// The crate-root-path predicate behind [`is_separate_test_target`],
/// split out as a pure function so the path arithmetic can be
/// unit-tested without a compiler context.
fn is_separate_target_path(path: &Path) -> bool {
    let parent = path.parent();
    // Flat form `<dir>/<name>.rs` — including `<dir>/main.rs`, a target
    // literally named `main` — roots directly in the target directory,
    // so check the immediate parent first. The subdirectory form
    // `<dir>/<name>/main.rs` roots one level deeper, so for a `main.rs`
    // leaf also accept the grandparent. Checking the parent first is
    // what keeps `tests/main.rs` matched instead of walking past `tests`
    // to nothing.
    is_target_directory(parent)
        || (path.file_name().and_then(|name| name.to_str()) == Some("main.rs")
            && is_target_directory(parent.and_then(Path::parent)))
}

/// Whether `dir`'s final component is one of Cargo's separate-target
/// directories (`tests/`, `benches/`, `examples/`).
fn is_target_directory(dir: Option<&Path>) -> bool {
    matches!(
        dir.and_then(Path::file_name).and_then(|name| name.to_str()),
        Some("tests" | "benches" | "examples"),
    )
}

pub(super) fn real_path(file: &SourceFile) -> Option<PathBuf> {
    match &file.name {
        FileName::Real(real) => real.local_path().map(Path::to_path_buf),
        _ => None,
    }
}

/// The canonical extraction target for a module of `name` declared in
/// `parent`: `<parent_dir>/<parent_stem>/<name>.rs` for an ordinary
/// file, or `<parent_dir>/<name>.rs` when the parent is a
/// directory-owning file (`lib.rs` / `main.rs` / `mod.rs`), matching
/// where Cargo places a child module of such a file. Used to name the
/// target in inline-style help text.
pub(super) fn canonical_target(parent: &Path, name: &str) -> Option<PathBuf> {
    let dir = parent.parent()?;
    if is_mod_root(parent) {
        return Some(dir.join(format!("{name}.rs")));
    }
    let stem = parent.file_stem()?.to_str()?;
    Some(dir.join(stem).join(format!("{name}.rs")))
}

fn is_mod_root(parent: &Path) -> bool {
    matches!(
        parent.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs" | "mod.rs"),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_separate_target_path;

    #[test]
    fn separate_targets_are_recognised() {
        // Flat `<dir>/<name>.rs`, the `<dir>/main.rs` named-`main` case,
        // the `<dir>/<name>/main.rs` subdirectory form, and an absolute
        // integration-test root — for tests, benches, and examples alike.
        for path in [
            "tests/integration.rs",
            "tests/main.rs",
            "tests/suite/main.rs",
            "benches/bench.rs",
            "benches/suite/main.rs",
            "examples/demo.rs",
            "examples/demo/main.rs",
            "/abs/tests/integration.rs",
        ] {
            assert!(
                is_separate_target_path(Path::new(path)),
                "`{path}` should be treated as a separate test target",
            );
        }
    }

    #[test]
    fn library_and_binary_roots_are_checked() {
        // Library, default binary, extra binaries (flat and the
        // `src/bin/<name>/main.rs` multi-file form), and a nested module
        // file all stay in scope.
        for path in [
            "src/lib.rs",
            "src/main.rs",
            "src/bin/cli.rs",
            "src/bin/cli/main.rs",
            "src/rules/inline_test_footprint/paths.rs",
            "lib.rs",
        ] {
            assert!(
                !is_separate_target_path(Path::new(path)),
                "`{path}` should be checked, not skipped",
            );
        }
    }
}
