//! External-module on-disk layout and unexpected-sibling checks, plus
//! the path arithmetic the inline-style help text shares.

use std::path::{Component, Path, PathBuf};

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Item, Mod};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{FileName, SourceFile, Span, Symbol};

use super::UNIT_TEST_FILE_LAYOUT;
use super::config::{ExternalLayout, UnitTestFileLayout};

/// Check an external `#[cfg(test)] mod <name>;` against the configured
/// `external_layout`, and (under `nested`) flag a stray flattened
/// sibling left over from a half-completed migration.
pub(super) fn check_external_mod(
    state: &UnitTestFileLayout,
    cx: &LateContext<'_>,
    item: &Item<'_>,
    name: Symbol,
    module: &Mod<'_>,
) {
    if let ExternalLayout::Any = state.external_layout {
        return;
    }
    let source_map = cx.sess().source_map();
    let Some(parent) = source_file_path(cx, item.span) else {
        return;
    };
    let child_file = source_map.lookup_source_file(module.spans.inner_span.lo());
    let Some(child) = real_path(&child_file) else {
        return;
    };
    let name = name.as_str();
    let Some(nested) = nested_target(&parent, name) else {
        return;
    };

    match state.external_layout {
        ExternalLayout::Nested => {
            if !same_path(&child, &nested) {
                span_lint_and_help(
                    cx,
                    UNIT_TEST_FILE_LAYOUT,
                    item.span,
                    "external test module is not in the canonical nested location",
                    None,
                    format!("move the test file to `{}`", nested.display()),
                );
            } else if state.flag_unexpected_sibling
                && let Some(sibling) = sibling_target(&parent, name)
                && !same_path(&sibling, &nested)
                && sibling.exists()
                && !is_loaded_module(cx, &sibling)
            {
                span_lint_and_help(
                    cx,
                    UNIT_TEST_FILE_LAYOUT,
                    item.span,
                    "an unexpected sibling test file exists alongside the nested test module",
                    None,
                    format!(
                        "delete `{}` or merge it into `{}`",
                        sibling.display(),
                        nested.display(),
                    ),
                );
            }
        }
        ExternalLayout::Sibling => {
            let sibling = sibling_target(&parent, name);
            let accepted = same_path(&child, &nested)
                || sibling.as_ref().is_some_and(|path| same_path(&child, path));
            if !accepted {
                let help = match &sibling {
                    Some(sibling) => format!(
                        "move the test file to `{}` or `{}`",
                        nested.display(),
                        sibling.display(),
                    ),
                    None => format!("move the test file to `{}`", nested.display()),
                };
                span_lint_and_help(
                    cx,
                    UNIT_TEST_FILE_LAYOUT,
                    item.span,
                    "external test module is not in an accepted location",
                    None,
                    help,
                );
            }
        }
        ExternalLayout::Any => {}
    }
}

/// Whether the crate currently being compiled is a *separate*
/// non-library target — an integration test (`tests/`), benchmark
/// (`benches/`), or example (`examples/`) crate — rather than the
/// library or binary whose unit-test layout this rule governs.
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
/// is `tests`, `benches`, or `examples`, while a library/binary roots
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
/// directories (`tests`, `benches`, `examples`).
fn is_target_directory(dir: Option<&Path>) -> bool {
    matches!(
        dir.and_then(Path::file_name).and_then(|name| name.to_str()),
        Some("tests" | "benches" | "examples"),
    )
}

fn source_file_path(cx: &LateContext<'_>, span: Span) -> Option<PathBuf> {
    let file = cx.sess().source_map().lookup_source_file(span.lo());
    real_path(&file)
}

pub(super) fn real_path(file: &SourceFile) -> Option<PathBuf> {
    match &file.name {
        FileName::Real(real) => real.local_path().map(Path::to_path_buf),
        _ => None,
    }
}

/// Whether `path` is a source file the compiler actually loaded. The
/// unexpected-sibling check only wants to flag *stragglers* — files
/// left on disk by a half-finished migration that no `mod` declaration
/// loads. A `<stem>_<name>.rs` that is itself a live module (declared
/// elsewhere as `mod <stem>_<name>;`) is in the source map, so it is
/// not a straggler and must not be flagged for deletion.
fn is_loaded_module(cx: &LateContext<'_>, path: &Path) -> bool {
    cx.sess()
        .source_map()
        .files()
        .iter()
        .filter_map(|file| real_path(file))
        .any(|loaded| same_path(&loaded, path))
}

/// The canonical extraction target for a module of `name` declared in
/// `parent`, per `layout`. Used both to judge `nested` placement and to
/// name the target in inline-style help text.
pub(super) fn canonical_target(
    parent: &Path,
    name: &str,
    layout: ExternalLayout,
) -> Option<PathBuf> {
    match layout {
        ExternalLayout::Sibling => sibling_target(parent, name),
        ExternalLayout::Nested | ExternalLayout::Any => nested_target(parent, name),
    }
}

/// `<parent_dir>/<parent_stem>/<name>.rs` for an ordinary file, or
/// `<parent_dir>/<name>.rs` when the parent is a directory-owning file
/// (`lib.rs` / `main.rs` / `mod.rs`), matching where Cargo places a
/// child module of such a file.
fn nested_target(parent: &Path, name: &str) -> Option<PathBuf> {
    let dir = parent.parent()?;
    if is_mod_root(parent) {
        return Some(dir.join(format!("{name}.rs")));
    }
    let stem = parent.file_stem()?.to_str()?;
    Some(dir.join(stem).join(format!("{name}.rs")))
}

/// `<parent_dir>/<parent_stem>_<name>.rs`, or `None` for a
/// directory-owning parent (`lib.rs` / `main.rs` / `mod.rs`): such a
/// file's children already live beside it as `<parent_dir>/<name>.rs`
/// (the nested form), so there is no distinct flattened sibling — a
/// `lib_<name>.rs` is a path Cargo never loads, and offering it as an
/// accepted location or flagging it as a stray would both be nonsense.
fn sibling_target(parent: &Path, name: &str) -> Option<PathBuf> {
    if is_mod_root(parent) {
        return None;
    }
    let dir = parent.parent()?;
    let stem = parent.file_stem()?.to_str()?;
    Some(dir.join(format!("{stem}_{name}.rs")))
}

fn is_mod_root(parent: &Path) -> bool {
    matches!(
        parent.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs" | "mod.rs"),
    )
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize(left) == normalize(right)
}

/// Drop `.` components so paths reported by the source map compare
/// equal to the targets we synthesise from a sibling's path regardless
/// of a leading `./`.
fn normalize(path: &Path) -> PathBuf {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
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
            "src/rules/unit_test_file_layout/layout.rs",
            "lib.rs",
        ] {
            assert!(
                !is_separate_target_path(Path::new(path)),
                "`{path}` should be checked, not skipped",
            );
        }
    }
}
