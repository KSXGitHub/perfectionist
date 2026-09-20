//! Whether the crate whose methods the diagnostic names is reachable
//! from the code under lint.
//!
//! Two questions, driving different things. Whether the compiler
//! loaded the crate decides whether the lint speaks at all, under the
//! `require_command_extra_dependency` knob. Whether the trait is
//! imported at the call decides only which remedy the diagnostic
//! names.

use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, Item, ItemKind, Node};
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// The crate name `command-extra` compiles under, as the compiler
/// spells it rather than as Cargo does.
const CRATE: &str = "command_extra";

/// The trait whose by-value setters the diagnostic names. Where it is
/// not imported, the diagnostic says to import it.
const TRAIT: &str = "CommandExtra";

/// Whether a crate named `command_extra` is among the loaded ones.
pub(super) fn crate_is_loaded(cx: &LateContext<'_>) -> bool {
    let wanted = Symbol::intern(CRATE);
    cx.tcx
        .crates(())
        .iter()
        .any(|&krate| cx.tcx.crate_name(krate) == wanted)
}

/// Whether the innermost module around `call` imports `CommandExtra`.
///
/// Scoped to that module because a trait has to be in scope where the
/// method is called, and a parent module's `use` does not reach a
/// child. A trait reached some other way -- a glob, a project prelude --
/// reads here as absent, which costs the reader a redundant "add the
/// import" line rather than anything load-bearing.
pub(super) fn trait_is_imported(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    let module = cx.tcx.parent_module(call.hir_id);
    let wanted_crate = Symbol::intern(CRATE);
    let wanted_trait = Symbol::intern(TRAIT);
    // The module's *direct* children. `hir_module_items` would also
    // reach items nested in bodies, and a `use` inside one function
    // does not bring the trait into scope for that function's siblings.
    let items = match cx.tcx.hir_node_by_def_id(module.to_local_def_id()) {
        Node::Crate(contents) => contents.item_ids,
        Node::Item(Item {
            kind: ItemKind::Mod(_, contents),
            ..
        }) => contents.item_ids,
        _ => return false,
    };
    items
        .iter()
        .filter_map(|item_id| match cx.tcx.hir_item(*item_id).kind {
            ItemKind::Use(path, _) => Some(path),
            _ => None,
        })
        .flat_map(|path| path.res.iter())
        .any(|res| match res {
            Some(Res::Def(DefKind::Trait, def_id)) => {
                cx.tcx.crate_name(def_id.krate) == wanted_crate
                    && cx.tcx.item_name(*def_id) == wanted_trait
            }
            _ => false,
        })
}
