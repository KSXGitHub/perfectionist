//! `forbid` style: every form that names a module through `self` in a
//! `use` statement is a violation, and the autofix rewrites it to the
//! bare module import.

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::indent_of;
use rustc_ast::{Item, UseTree, UseTreeKind};
use rustc_errors::Applicability;
use rustc_lint::EarlyContext;
use rustc_span::Span;

use super::SELF_IMPORT;
use super::render::{
    attr_snippets, is_self_leaf, real_segments, render_prefix, render_rooted, render_use_tree,
    render_visibility, segment_names, simple_self_module, with_rename,
};

const MESSAGE: &str = "this `use` imports a module through `self`";
const SUGGESTION_LABEL: &str = "import the module by its bare path";

/// Check one top-level `use` item under `forbid` style. `tree` is the
/// item's own `use` tree.
pub(super) fn check_use_item(cx: &EarlyContext<'_>, item: &Item, tree: &UseTree) {
    visit(cx, item, tree, true);
}

/// Walk `node` (a `use` tree, possibly nested), emitting a diagnostic
/// for every `self`-as-module form. `at_root` marks the item's own
/// top-level tree, where a `{self, X}` group splits into two separate
/// `use` statements; nested groups are rewritten in place.
fn visit(cx: &EarlyContext<'_>, item: &Item, node: &UseTree, at_root: bool) {
    match &node.kind {
        UseTreeKind::Simple(rename) => {
            if let Some(module) = simple_self_module(node)
                && !module.is_empty()
            {
                // `a::b::self` — names module `a::b` through a trailing
                // `self`. (The bare `{self}` leaf has an empty module
                // here and is rewritten by its enclosing brace group.)
                // Drop the final `self` segment and render the rest,
                // keeping any leading `::`.
                let segments = real_segments(&node.prefix);
                let module_path = render_rooted(&node.prefix, &segments[..segments.len() - 1]);
                emit_replacement(
                    cx,
                    node.span(),
                    with_rename(module_path, *rename),
                    Some(
                        "the trailing `self` is redundant here — it re-names the module the \
                         path already gives; import it directly",
                    ),
                );
            }
        }
        UseTreeKind::Nested { items, .. } => {
            if let Some(self_idx) = items.iter().position(|(child, _)| is_self_leaf(child)) {
                rewrite_self_group(cx, item, node, items, self_idx, at_root);
            }
            // Recurse into the non-`self` members to catch `self` forms
            // nested deeper inside the tree. The `self` leaf itself
            // names this node's module and is handled above.
            for (child, _) in items {
                if !is_self_leaf(child) {
                    visit(cx, item, child, false);
                }
            }
        }
        UseTreeKind::Glob(_) => {}
    }
}

/// Rewrite a brace group that contains a bare `self` leaf. With no
/// other members it collapses to the bare module path; with other
/// members the module import is lifted out — into a separate statement
/// at the item root, or a sibling brace-list entry when nested.
fn rewrite_self_group(
    cx: &EarlyContext<'_>,
    item: &Item,
    node: &UseTree,
    items: &[(UseTree, rustc_ast::NodeId)],
    self_idx: usize,
    at_root: bool,
) {
    if segment_names(&node.prefix).is_empty() {
        // `use {self, ...};` with no prefix names nothing coherent;
        // leave it alone rather than emit a broken rewrite.
        return;
    }
    let self_rename = match items[self_idx].0.kind {
        UseTreeKind::Simple(rename) => rename,
        _ => None,
    };
    let others: Vec<&UseTree> = items
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != self_idx)
        .map(|(_, (child, _))| child)
        .collect();
    let base = render_prefix(&node.prefix);
    let module = with_rename(base.clone(), self_rename);

    if others.is_empty() {
        // `use prefix::{self};` -> `use prefix;`
        emit_replacement(cx, node.span(), module, None);
        return;
    }

    let rest = render_rest(&base, &others);
    if at_root {
        // `use prefix::{self, X};` -> two statements. Replacing only the
        // tree span keeps the original `use`, visibility, attributes,
        // and the trailing `;` (which terminates the injected second
        // statement). The injected statement replays the item's
        // attributes and visibility so neither is dropped.
        let visibility = render_visibility(cx, item);
        let indent = " ".repeat(indent_of(cx, item.span).unwrap_or(0));
        let attrs: String = attr_snippets(cx, item)
            .iter()
            .map(|attr| format!("{attr}\n{indent}"))
            .collect();
        let replacement = format!("{module};\n{indent}{attrs}{visibility}use {rest}");
        emit_replacement(cx, node.span(), replacement, None);
    } else {
        // Nested `prefix::{self, X}` sits in a comma-separated parent
        // group, so it can expand in place: `prefix, prefix::{X}`.
        emit_replacement(cx, node.span(), format!("{module}, {rest}"), None);
    }
}

/// Render the non-`self` members of a brace group under their shared
/// module path `base`: `base::X` for one member, `base::{X, Y}` for
/// more.
fn render_rest(base: &str, others: &[&UseTree]) -> String {
    if let [single] = others {
        format!("{base}::{}", render_use_tree(single))
    } else {
        let inner = others
            .iter()
            .map(|tree| render_use_tree(tree))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{base}::{{{inner}}}")
    }
}

/// Emit the lint at `span` with a `MaybeIncorrect` suggestion. Every
/// `forbid` rewrite is `MaybeIncorrect`: the bare form imports every
/// namespace named by the final segment, while the `self` form imports
/// only the module — a difference that surfaces in the rare case where
/// a value or macro shares the module's name in the same parent.
fn emit_replacement(
    cx: &EarlyContext<'_>,
    span: Span,
    replacement: String,
    note: Option<&'static str>,
) {
    span_lint_and_then(cx, SELF_IMPORT, span, MESSAGE, |diag| {
        if let Some(note) = note {
            diag.note(note);
        }
        diag.span_suggestion(
            span,
            SUGGESTION_LABEL,
            replacement,
            Applicability::MaybeIncorrect,
        );
    });
}
