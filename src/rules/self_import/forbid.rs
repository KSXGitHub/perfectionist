//! `forbid` style: every form that names a module through `self` in a
//! `use` statement is a violation, and the autofix rewrites it to the
//! bare module import.

use clippy_utils::source::snippet_indent;
use rustc_ast::{Item, UseTree, UseTreeKind};
use rustc_lint::LateContext;
use rustc_span::Span;

use super::render::{
    attr_snippets, is_self_leaf, real_segments, render_prefix, render_rooted, render_use_tree,
    render_visibility, segment_names, simple_self_module, with_rename,
};
use super::{Fix, Pending};

const MESSAGE: &str = "this `use` imports a module through `self`";
// Suggestion labels, chosen per rewrite shape so the help text matches
// the actual replacement.
const LABEL_BARE_PATH: &str = "import the module by its bare path";
const LABEL_SPLIT: &str = "split the module import into its own `use` statement";
const LABEL_EXPAND: &str = "import the module alongside the group's other items";

/// Check one top-level `use` item under `forbid` style. `tree` is the
/// item's own `use` tree. Detected violations are parked in `violations`
/// for deferred, HIR-anchored emission.
pub(super) fn check_use_item(
    cx: &LateContext<'_>,
    item: &Item,
    tree: &UseTree,
    violations: &mut Vec<Pending>,
) {
    visit(cx, item, tree, true, violations);
}

/// Walk `node` (a `use` tree, possibly nested), parking a violation for
/// every `self`-as-module form. `at_root` marks the item's own
/// top-level tree, where a `{self, X}` group splits into two separate
/// `use` statements; nested groups are rewritten in place.
fn visit(
    cx: &LateContext<'_>,
    item: &Item,
    node: &UseTree,
    at_root: bool,
    violations: &mut Vec<Pending>,
) {
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
                    item,
                    node.span(),
                    LABEL_BARE_PATH,
                    with_rename(module_path, *rename),
                    Some(
                        "the trailing `self` is redundant here — it re-names the module the \
                         path already gives; import it directly",
                    ),
                    violations,
                );
            }
        }
        UseTreeKind::Nested { items, .. } => {
            if let Some(self_idx) = items.iter().position(|(child, _)| is_self_leaf(child)) {
                rewrite_self_group(cx, item, node, items, self_idx, at_root, violations);
            }
            // Recurse into the non-`self` members to catch `self` forms
            // nested deeper inside the tree. The `self` leaf itself
            // names this node's module and is handled above.
            for (child, _) in items {
                if !is_self_leaf(child) {
                    visit(cx, item, child, false, violations);
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
    cx: &LateContext<'_>,
    item: &Item,
    node: &UseTree,
    items: &[(UseTree, rustc_ast::NodeId)],
    self_idx: usize,
    at_root: bool,
    violations: &mut Vec<Pending>,
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
    if others.iter().any(|tree| contains_self_form(tree)) {
        // A non-`self` sibling carries its own `self` form. Rewriting
        // this group now would (a) emit a suggestion whose span contains
        // the sibling's own — overlapping rewrites rustfix can't both
        // apply in one pass — and (b) reproduce that sibling verbatim, so
        // the "fixed" text would still import a module through `self`.
        // Leave this group for a later pass: the recursion in `visit`
        // rewrites the inner form first, and once it is gone this group is
        // rewritten cleanly.
        return;
    }
    let base = render_prefix(&node.prefix);
    let module = with_rename(base.clone(), self_rename);

    if others.is_empty() {
        // `use prefix::{self};` -> `use prefix;`
        emit_replacement(item, node.span(), LABEL_BARE_PATH, module, None, violations);
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
        let indent = snippet_indent(cx, item.span).unwrap_or_default();
        let attrs: String = attr_snippets(cx, item)
            .iter()
            .map(|attr| format!("{attr}\n{indent}"))
            .collect();
        let replacement = format!("{module};\n{indent}{attrs}{visibility}use {rest}");
        emit_replacement(
            item,
            node.span(),
            LABEL_SPLIT,
            replacement,
            None,
            violations,
        );
    } else {
        // Nested `prefix::{self, X}` sits in a comma-separated parent
        // group, so it can expand in place: `prefix, prefix::{X}`.
        emit_replacement(
            item,
            node.span(),
            LABEL_EXPAND,
            format!("{module}, {rest}"),
            None,
            violations,
        );
    }
}

/// Whether `tree` contains a `self`-as-module form this rule rewrites —
/// a `{self}` / `{self, ...}` brace group or a trailing `x::self` — at any
/// depth. Used to detect a brace group whose non-`self` sibling carries
/// its own `self`, where rewriting the outer group would overlap the
/// inner rewrite (see [`rewrite_self_group`]).
fn contains_self_form(tree: &UseTree) -> bool {
    match &tree.kind {
        // A trailing `x::self`; the bare `{self}` leaf (empty module) is
        // caught by its enclosing group's `is_self_leaf` arm below.
        UseTreeKind::Simple(_) => simple_self_module(tree).is_some_and(|module| !module.is_empty()),
        UseTreeKind::Nested { items, .. } => items
            .iter()
            .any(|(child, _)| is_self_leaf(child) || contains_self_form(child)),
        UseTreeKind::Glob(_) => false,
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

/// Park a violation at `span` with a `MaybeIncorrect` single-span
/// suggestion. Every `forbid` rewrite is `MaybeIncorrect`: the bare form
/// imports every namespace named by the final segment, while the `self`
/// form imports only the module — a difference that surfaces in the rare
/// case where a value or macro shares the module's name in the same
/// parent. The lint-level anchor is the enclosing `use` item's span (an
/// out-of-line module's `use` lives in its own file, so the item span is
/// always a safe anchor for HIR resolution).
fn emit_replacement(
    item: &Item,
    span: Span,
    label: &'static str,
    replacement: String,
    note: Option<&'static str>,
    violations: &mut Vec<Pending>,
) {
    violations.push(Pending {
        anchor: item.span,
        span,
        message: MESSAGE,
        fix: Fix::Replace {
            label,
            replacement,
            note,
        },
    });
}
