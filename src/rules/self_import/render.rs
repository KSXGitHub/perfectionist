//! Shared helpers for rendering `use` trees back to source text and
//! recognising the `self` forms this rule rewrites.

use clippy_utils::source::snippet;
use rustc_ast::{Item, Path, PathSegment, UseTree, UseTreeKind, VisibilityKind};
use rustc_lint::LateContext;
use rustc_span::{Ident, Symbol, kw};

/// A path's segments with the synthetic leading `{{root}}` (`::`)
/// segment stripped. `PathRoot`, when present, is always first, so
/// dropping it leaves the segments aligned 1-to-1 with
/// [`segment_names`].
pub(super) fn real_segments(path: &Path) -> &[PathSegment] {
    match path.segments.as_slice() {
        [first, rest @ ..] if first.ident.name == kw::PathRoot => rest,
        all => all,
    }
}

/// Whether the path is rooted at a leading `::` (a `{{root}}`
/// `PathRoot` segment). The leading `::` is load-bearing: `::foo` names
/// an external crate, while `foo` is resolved through the extern
/// prelude / crate-relative, and the two can differ when a local item
/// shadows a crate name. Rewrites must preserve it, and `combined` must
/// not treat `::foo` and `foo` as the same module.
pub(super) fn has_path_root(path: &Path) -> bool {
    matches!(path.segments.first(), Some(first) if first.ident.name == kw::PathRoot)
}

/// Segment names of a path, dropping the synthetic `{{root}}`
/// leading-`::` segment so `::foo::bar` and `foo::bar` compare
/// identically. Used for the rule's structural comparisons; rendering
/// goes through [`render_segments`] instead, which is raw-identifier
/// aware.
pub(super) fn segment_names(path: &Path) -> Vec<Symbol> {
    real_segments(path)
        .iter()
        .map(|segment| segment.ident.name)
        .collect()
}

/// Render path segments as `a::b::c`, through each segment's `Ident`
/// rather than its `Symbol`. `Ident`'s `Display` re-adds the `r#`
/// raw-identifier prefix where the source had it (and the edition
/// makes it a keyword), so a module named `r#match` renders as
/// `r#match`, not the bare keyword `match` — which would produce an
/// autofix that fails to parse.
pub(super) fn render_segments(segments: &[PathSegment]) -> String {
    segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Render `segments` (already `PathRoot`-stripped) with `path`'s
/// leading `::` restored when it had one. Use this — not bare
/// [`render_segments`] — whenever the rendered text is the *start* of a
/// path, so an absolute `::foo` import is not silently rewritten to the
/// crate-relative `foo`.
pub(super) fn render_rooted(path: &Path, segments: &[PathSegment]) -> String {
    let body = render_segments(segments);
    if has_path_root(path) {
        format!("::{body}")
    } else {
        body
    }
}

/// Render a path's full prefix (root and all segments).
pub(super) fn render_prefix(path: &Path) -> String {
    render_rooted(path, real_segments(path))
}

/// Append an optional `as rename` to an already-rendered path. The
/// rename is rendered through its `Ident` so a raw alias keeps `r#`.
pub(super) fn with_rename(path: String, rename: Option<Ident>) -> String {
    match rename {
        Some(rename) => format!("{path} as {rename}"),
        None => path,
    }
}

/// Render a `use` tree back to canonical source text (normalised
/// spacing). Renames become `path as rename`, globs `path::*`, and
/// nested groups `path::{a, b}`.
pub(super) fn render_use_tree(tree: &UseTree) -> String {
    let path = render_prefix(&tree.prefix);
    match &tree.kind {
        UseTreeKind::Simple(rename) => with_rename(path, *rename),
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
pub(super) fn render_visibility(cx: &LateContext<'_>, item: &Item) -> String {
    match item.vis.kind {
        VisibilityKind::Inherited => String::new(),
        _ => format!("{} ", snippet(cx, item.vis.span, "").trim()),
    }
}

/// Source snippets of an item's outer attributes, in source order.
/// Used both to require two imports' attributes to match before
/// folding them and to replay the attributes onto a statement a
/// rewrite synthesises. The rule re-parses source in a late pass, so
/// `#[cfg(...)]` attributes are *not* stripped here — they appear in
/// this list like any other attribute. That is what lets the fold's
/// attribute-equality check refuse to merge two imports differing only
/// in their `#[cfg(...)]`, and lets a `forbid` split replay the gate
/// onto both halves.
pub(super) fn attr_snippets(cx: &LateContext<'_>, item: &Item) -> Vec<String> {
    item.attrs
        .iter()
        .map(|attr| snippet(cx, attr.span, "").trim().to_string())
        .collect()
}
