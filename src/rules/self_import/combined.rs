//! `combined` style: adjacent imports of a module and an item from
//! that module fold into a single `use module::{self, item};`.

use super::render::{
    attr_snippets, has_path_root, is_self_leaf, real_segments, render_prefix, render_segments,
    render_use_tree, render_visibility, segment_names, with_rename,
};
use super::{Fix, Pending};
use rustc_ast::{Item, ItemKind, UseTree, UseTreeKind};
use rustc_errors::Applicability;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{Symbol, kw};

/// Scan an ordered sequence of items for adjacent module + item
/// imports to fold. Each entry is `Some(item)` for an item in source
/// order, or `None` for an intervening non-item statement (a `let`,
/// an expression) that breaks adjacency. A non-`use` item — or a `use`
/// from a macro expansion — also breaks the window.
pub(super) fn scan<'ast>(
    cx: &LateContext<'_>,
    entries: impl Iterator<Item = Option<&'ast Item>>,
    violations: &mut Vec<Pending>,
) {
    let mut previous: Option<&Item> = None;
    for entry in entries {
        match entry {
            Some(item) if matches!(item.kind, ItemKind::Use(_)) && !item.span.from_expansion() => {
                if let Some(first) = previous
                    && try_fold(cx, first, item, violations)
                {
                    previous = None;
                } else {
                    previous = Some(item);
                }
            }
            _ => previous = None,
        }
    }
}

/// Attempt to fold the adjacent pair `first` then `second`. Returns
/// whether a fold was emitted. The two must share visibility and have
/// matching attributes; one must import a module and the other an item
/// from that same module (in either order).
fn try_fold(
    cx: &LateContext<'_>,
    first: &Item,
    second: &Item,
    violations: &mut Vec<Pending>,
) -> bool {
    if render_visibility(cx, first) != render_visibility(cx, second) {
        return false;
    }
    if attr_snippets(cx, first) != attr_snippets(cx, second) {
        return false;
    }
    let (ItemKind::Use(first_tree), ItemKind::Use(second_tree)) = (&first.kind, &second.kind)
    else {
        return false;
    };
    // `::foo` and `foo` can resolve to different modules, so never fold
    // across a leading-`::` mismatch.
    if has_path_root(&first_tree.prefix) != has_path_root(&second_tree.prefix) {
        return false;
    }
    // The module import can be either statement; the item import is the
    // other. Whichever order, the folded statement is placed at `first`
    // and `second` is deleted.
    if let Some((module, bare)) = module_import(first_tree)
        && let Some(tail) = item_tail_under(second_tree, &module)
    {
        emit_fold(cx, first, second, first_tree, bare, &tail, violations);
        return true;
    }
    if let Some((module, bare)) = module_import(second_tree)
        && let Some(tail) = item_tail_under(first_tree, &module)
    {
        emit_fold(cx, first, second, second_tree, bare, &tail, violations);
        return true;
    }
    false
}

/// If `tree` imports a module and nothing else, the module's path
/// segments and whether it is the bare `use module;` form (`true`) as
/// opposed to `use module::{self};` (`false`). The bareness drives the
/// fix's applicability: when the source is a bare `use module;`, folding
/// it to `{self, ...}` narrows the import to just the module namespace
/// (dropping any value or macro of the same name), so that fold is
/// `MaybeIncorrect`. The `{self}` source form already imports only the
/// module, so folding it changes no namespace and stays
/// `MachineApplicable`.
fn module_import(tree: &UseTree) -> Option<(Vec<Symbol>, bool)> {
    match &tree.kind {
        UseTreeKind::Simple(None) => {
            let names = segment_names(&tree.prefix);
            // A trailing `self` (`use foo::self;`) is the `forbid`
            // rule's concern; don't treat it as a module import here.
            if names.is_empty() || names.last() == Some(&kw::SelfLower) {
                return None;
            }
            Some((names, true))
        }
        UseTreeKind::Nested { items, .. } => {
            // Exactly `use module::{self};`. A renamed `{self as x}`
            // changes the local name, so folding would drop the rename;
            // leave it alone.
            let [(only, _)] = items.as_slice() else {
                return None;
            };
            if !is_self_leaf(only) || matches!(only.kind, UseTreeKind::Simple(Some(_))) {
                return None;
            }
            let names = segment_names(&tree.prefix);
            if names.is_empty() {
                return None;
            }
            Some((names, false))
        }
        _ => None,
    }
}

/// If `tree` imports one or more items strictly under `module`, the
/// tail to splice after `self` (e.g. `Baz`, `Baz as Q`, `a, b`,
/// `sub::{a, b}`). `None` when `tree` is not under `module`, is a glob,
/// or already carries a `self`.
fn item_tail_under(tree: &UseTree, module: &[Symbol]) -> Option<String> {
    let names = segment_names(&tree.prefix);
    if names.len() < module.len() || &names[..module.len()] != module {
        return None;
    }
    let rest_names = &names[module.len()..];
    if rest_names.contains(&kw::SelfLower) {
        return None;
    }
    let rest_segments = &real_segments(&tree.prefix)[module.len()..];
    match &tree.kind {
        UseTreeKind::Simple(rename) => {
            // `use module::rest` — rest must be non-empty (otherwise
            // `tree` is the module itself, not an item under it).
            if rest_segments.is_empty() {
                return None;
            }
            Some(with_rename(render_segments(rest_segments), *rename))
        }
        UseTreeKind::Nested { items, .. } => {
            if items.iter().any(|(child, _)| is_self_leaf(child)) {
                return None;
            }
            let inner = items
                .iter()
                .map(|(child, _)| render_use_tree(child))
                .collect::<Vec<_>>()
                .join(", ");
            if rest_segments.is_empty() {
                // `use module::{a, b}` -> tail `a, b`
                Some(inner)
            } else {
                // `use module::sub::{a, b}` -> tail `sub::{a, b}`
                Some(format!("{}::{{{inner}}}", render_segments(rest_segments)))
            }
        }
        UseTreeKind::Glob(_) => None,
    }
}

/// Emit the fold suggestion: replace `first`'s tree with
/// `module::{self, tail}` and delete `second`'s whole statement.
/// `module_tree` is whichever of the two statements is the module
/// import — its prefix renders the `module` path (raw-identifier aware).
/// `first` is always a `use` item (the caller checked), so its own tree
/// is recovered here rather than threaded through a separate argument.
fn emit_fold(
    cx: &LateContext<'_>,
    first: &Item,
    second: &Item,
    module_tree: &UseTree,
    bare: bool,
    tail: &str,
    violations: &mut Vec<Pending>,
) {
    let ItemKind::Use(first_tree) = &first.kind else {
        return;
    };
    let module = render_prefix(&module_tree.prefix);
    let folded = format!("{module}::{{self, {tail}}}");
    let delete = first.span.shrink_to_hi().to(second.span);
    let source_map = cx.sess().source_map();
    // The fold edits two regions: it replaces `first`'s tree with
    // `folded` and deletes everything from the end of `first`'s
    // statement through the end of `second` (the whitespace gap, any
    // attributes on `second`, and all of `second`'s statement). Anything
    // dropped by either edit that an auto-applied fix shouldn't silently
    // discard downgrades the fold to `MaybeIncorrect`:
    //   - a non-blank gap up to `second`'s `use` keyword (`second.span`
    //     excludes outer attributes), i.e. a comment or an attribute on
    //     `second`;
    //   - a comment anywhere in the deleted region, including one *inside*
    //     `second`'s statement (between its `use` keyword and `;`), which
    //     the gap probe alone would miss;
    //   - a comment inside `first`'s tree, which the replacement edit
    //     discards (a `use` tree contains no `//` or `/*` except a comment).
    let gap = source_map
        .span_to_snippet(first.span.between(second.span))
        .unwrap_or_default();
    let deleted = source_map.span_to_snippet(delete).unwrap_or_default();
    let replaced = source_map
        .span_to_snippet(first_tree.span())
        .unwrap_or_default();
    let has_comment = [&deleted, &replaced]
        .iter()
        .any(|text| text.contains("//") || text.contains("/*"));
    let applicability = if bare || !gap.trim().is_empty() || has_comment {
        Applicability::MaybeIncorrect
    } else {
        Applicability::MachineApplicable
    };
    violations.push(Pending {
        anchor: first.span,
        span: first.span,
        message: "adjacent module and item imports can be combined through `self`",
        fix: Fix::Multipart {
            label: "combine into a single `use` with `self`",
            parts: vec![(first_tree.span(), folded), (delete, String::new())],
            applicability,
        },
    });
}
