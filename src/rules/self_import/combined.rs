//! `combined` style: adjacent imports of a module and an item from
//! that module fold into a single `use module::{self, item};`.

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{Item, ItemKind, UseTree, UseTreeKind};
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{Symbol, kw};

use super::SELF_IMPORT;
use super::render::{is_self_leaf, render_path, render_use_tree, render_visibility, segment_names};

/// Scan an ordered sequence of items for adjacent module + item
/// imports to fold. Each entry is `Some(item)` for an item in source
/// order, or `None` for an intervening non-item statement (a `let`,
/// an expression) that breaks adjacency. A non-`use` item — or a `use`
/// from a macro expansion — also breaks the window.
pub(super) fn scan<'ast>(cx: &EarlyContext<'_>, entries: impl Iterator<Item = Option<&'ast Item>>) {
    let mut previous: Option<&Item> = None;
    for entry in entries {
        match entry {
            Some(item) if matches!(item.kind, ItemKind::Use(_)) && !item.span.from_expansion() => {
                if let Some(first) = previous
                    && try_fold(cx, first, item)
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
/// whether a fold was emitted. The two must share visibility and carry
/// no attributes; one must import a module and the other an item from
/// that same module (in either order).
fn try_fold(cx: &EarlyContext<'_>, first: &Item, second: &Item) -> bool {
    if !first.attrs.is_empty() || !second.attrs.is_empty() {
        return false;
    }
    if render_visibility(cx, first) != render_visibility(cx, second) {
        return false;
    }
    let (ItemKind::Use(first_tree), ItemKind::Use(second_tree)) = (&first.kind, &second.kind)
    else {
        return false;
    };
    // The module import can be either statement; the item import is the
    // other. Whichever order, the folded statement is placed at `first`
    // and `second` is deleted.
    let fold = module_import(first_tree)
        .and_then(|(module, bare)| {
            item_tail_under(second_tree, &module).map(|tail| (module, bare, tail))
        })
        .or_else(|| {
            module_import(second_tree).and_then(|(module, bare)| {
                item_tail_under(first_tree, &module).map(|tail| (module, bare, tail))
            })
        });
    let Some((module, bare, tail)) = fold else {
        return false;
    };
    emit_fold(cx, first, second, first_tree, &module, bare, &tail);
    true
}

/// If `tree` imports a module and nothing else, the module's path
/// segments and whether it is the bare `use module;` form (`true`) as
/// opposed to `use module::{self};` (`false`). The bareness drives the
/// fix's applicability: bare narrows the import to the module
/// namespace, so it is only `MaybeIncorrect`.
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
    let rest = &names[module.len()..];
    if rest.contains(&kw::SelfLower) {
        return None;
    }
    match &tree.kind {
        UseTreeKind::Simple(rename) => {
            // `use module::rest` — rest must be non-empty (otherwise
            // `tree` is the module itself, not an item under it).
            if rest.is_empty() {
                return None;
            }
            let base = render_path(rest);
            Some(match rename {
                Some(rename) => format!("{base} as {}", rename.name),
                None => base,
            })
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
            if rest.is_empty() {
                // `use module::{a, b}` -> tail `a, b`
                Some(inner)
            } else {
                // `use module::sub::{a, b}` -> tail `sub::{a, b}`
                Some(format!("{}::{{{inner}}}", render_path(rest)))
            }
        }
        UseTreeKind::Glob(_) => None,
    }
}

/// Emit the fold suggestion: replace `first`'s tree with
/// `module::{self, tail}` and delete `second`'s whole statement.
fn emit_fold(
    cx: &EarlyContext<'_>,
    first: &Item,
    second: &Item,
    first_tree: &UseTree,
    module: &[Symbol],
    bare: bool,
    tail: &str,
) {
    let folded = format!("{}::{{self, {tail}}}", render_path(module));
    // The deletion removes everything from the end of `first`'s
    // statement through the end of `second`'s — the whitespace gap and
    // all of `second`. If a comment sits in that gap the deletion would
    // discard it, so a non-blank gap downgrades to `MaybeIncorrect`.
    let gap = cx
        .sess()
        .source_map()
        .span_to_snippet(first.span.between(second.span))
        .unwrap_or_default();
    let applicability = if bare || !gap.trim().is_empty() {
        Applicability::MaybeIncorrect
    } else {
        Applicability::MachineApplicable
    };
    let delete = first.span.shrink_to_hi().to(second.span);
    span_lint_and_then(
        cx,
        SELF_IMPORT,
        first.span,
        "adjacent module and item imports can be combined through `self`",
        |diag| {
            diag.multipart_suggestion(
                "combine into a single `use` with `self`",
                vec![(first_tree.span(), folded.clone()), (delete, String::new())],
                applicability,
            );
        },
    );
}
