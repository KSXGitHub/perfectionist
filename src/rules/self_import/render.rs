//! Shared helpers for rendering `use` trees back to source text and
//! recognising the `self` forms this rule rewrites.

use clippy_utils::source::snippet;
use rustc_ast::{Item, Path, PathSegment, UseTree, UseTreeKind, VisibilityKind};
use rustc_lint::EarlyContext;
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
    let path = render_segments(real_segments(&tree.prefix));
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
pub(super) fn render_visibility(cx: &EarlyContext<'_>, item: &Item) -> String {
    match item.vis.kind {
        VisibilityKind::Inherited => String::new(),
        _ => format!("{} ", snippet(cx, item.vis.span, "").trim()),
    }
}

/// Source snippets of an item's outer attributes, in source order.
/// Used both to require two imports' attributes to match before
/// folding them and to replay the attributes onto a statement a
/// rewrite synthesises. Note that `#[cfg(...)]` attributes are already
/// stripped by the time this early pass runs, so in practice these are
/// doc comments, lint-level attributes, and tool attributes.
pub(super) fn attr_snippets(cx: &EarlyContext<'_>, item: &Item) -> Vec<String> {
    item.attrs
        .iter()
        .map(|attr| snippet(cx, attr.span, "").trim().to_string())
        .collect()
}
