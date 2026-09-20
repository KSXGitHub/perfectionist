//! Whether the crate whose methods the diagnostic names is reachable
//! from the code under lint.
//!
//! The answers drive different things. Whether the crate is a declared
//! dependency decides whether the lint speaks at all, under the
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

/// Whether the crate under lint declares a dependency on
/// `command_extra`.
///
/// Not whether the compiler *loaded* it, which is a different question
/// with the wrong answer both ways round. `--extern` is lazy, so a
/// declared dependency nothing has named yet is absent from
/// `tcx.crates(())` -- and that is the crate the advice most applies
/// to. A crate reached only through another crate's signature is
/// present there, though this crate cannot name it, and the diagnostic
/// would then offer an import that is `E0432`.
///
/// A crate this one can name arrived one of two ways, and which depends
/// on how the compiler was driven rather than on anything the author
/// wrote. Cargo passes `--extern command_extra=...`, recorded whether
/// or not the crate is ever named. A crate written with
/// `extern crate command_extra;` records the item instead, which is
/// what the fixtures under `ui/` do.
pub(super) fn crate_is_declared(cx: &LateContext<'_>) -> bool {
    if cx
        .tcx
        .sess
        .opts
        .externs
        .get(CRATE)
        .is_some_and(|entry| entry.add_prelude)
    {
        return true;
    }
    let wanted = Symbol::intern(CRATE);
    cx.tcx
        .resolutions(())
        .extern_crate_map
        .items()
        .any(|(_, krate)| cx.tcx.crate_name(*krate) == wanted)
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
