//! Shared helpers for rendering `use` trees back to source text and
//! recognising the `self` forms this rule rewrites.

use clippy_utils::source::snippet;
use rustc_ast::{Item, Path, UseTree, UseTreeKind, VisibilityKind};
use rustc_lint::EarlyContext;
use rustc_span::{Symbol, kw};

/// Segment names of a path, dropping the synthetic `{{root}}`
/// leading-`::` segment so `::foo::bar` and `foo::bar` render
/// identically.
pub(super) fn segment_names(path: &Path) -> Vec<Symbol> {
    path.segments
        .iter()
        .map(|segment| segment.ident.name)
        .filter(|name| *name != kw::PathRoot)
        .collect()
}

/// Render a list of path segment names as `a::b::c`.
pub(super) fn render_path(names: &[Symbol]) -> String {
    names
        .iter()
        .map(Symbol::to_string)
        .collect::<Vec<_>>()
        .join("::")
}

/// Render a `use` tree back to canonical source text (normalised
/// spacing). Renames become `path as rename`, globs `path::*`, and
/// nested groups `path::{a, b}`.
pub(super) fn render_use_tree(tree: &UseTree) -> String {
    let path = render_path(&segment_names(&tree.prefix));
    match &tree.kind {
        UseTreeKind::Simple(None) => path,
        UseTreeKind::Simple(Some(rename)) => format!("{path} as {}", rename.name),
        UseTreeKind::Glob(_) => {
            if path.is_empty() {
                "*".to_owned()
            } else {
                format!("{path}::*")
            }
        }
        UseTreeKind::Nested { items, .. } => {
            let inner = items
                .iter()
                .map(|(item, _)| render_use_tree(item))
                .collect::<Vec<_>>()
                .join(", ");
            if path.is_empty() {
                format!("{{{inner}}}")
            } else {
                format!("{path}::{{{inner}}}")
            }
        }
    }
}

/// Whether `tree` is a bare `self` leaf — the `{self}` brace-group
/// member, whose prefix is exactly one `self` segment. Distinguished
/// from the `a::b::self` trailing form (prefix length >= 2), which
/// names a module relative to the child's own prefix rather than its
/// parent's.
pub(super) fn is_self_leaf(tree: &UseTree) -> bool {
    matches!(tree.kind, UseTreeKind::Simple(_))
        && matches!(segment_names(&tree.prefix).as_slice(), [name] if *name == kw::SelfLower)
}

/// For a `Simple` node whose prefix's last segment is `self`, the
/// module it imports: the prefix minus that trailing `self`. `None`
/// when the node is not a `self`-terminated simple import. The bare
/// `{self}` leaf (prefix exactly `[self]`) returns `Some(empty)` —
/// callers treat the empty result as "the parent module" and leave
/// the rewrite to the enclosing brace group.
pub(super) fn simple_self_module(tree: &UseTree) -> Option<Vec<Symbol>> {
    if !matches!(tree.kind, UseTreeKind::Simple(_)) {
        return None;
    }
    let names = segment_names(&tree.prefix);
    match names.split_last() {
        Some((last, rest)) if *last == kw::SelfLower => Some(rest.to_vec()),
        _ => None,
    }
}

/// The item's visibility rendered with a trailing space (`"pub "`,
/// `"pub(crate) "`), or the empty string for inherited visibility. Used
/// when a rewrite has to synthesise a fresh `use` statement that keeps
/// the original's visibility.
pub(super) fn render_visibility(cx: &EarlyContext<'_>, item: &Item) -> String {
    match item.vis.kind {
        VisibilityKind::Inherited => String::new(),
        _ => format!("{} ", snippet(cx, item.vis.span, "").trim()),
    }
}
