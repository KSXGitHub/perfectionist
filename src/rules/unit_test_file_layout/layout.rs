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

/// `<parent_dir>/<parent_stem>_<name>.rs`.
fn sibling_target(parent: &Path, name: &str) -> Option<PathBuf> {
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
