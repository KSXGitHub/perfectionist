//! Whether the crate whose methods the diagnostic names is reachable
//! from the code under lint.
//!
//! Neither answer is the whole of it on its own. Whether the crate is a
//! declared dependency is what the `require_command_extra_dependency`
//! knob gates on, and whether the trait is imported at the call picks
//! which remedy the diagnostic names -- but an import is also evidence
//! of a dependency the declared set cannot see, so the caller reads it
//! for both.

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

/// Whether the crate under lint declares a dependency named
/// `command_extra`.
///
/// Not whether the compiler *loaded* it, which is a different question
/// with the wrong answer both ways round. `--extern` is lazy, so a
/// declared dependency nothing has named yet is absent from
/// `tcx.crates(())` -- and that is the crate the advice most applies
/// to. A crate reached only through another crate's signature is
/// present there, though this crate cannot name it at all.
///
/// Where the answer lives depends on how the compiler was driven rather
/// than on anything the author wrote. Cargo passes
/// `--extern <manifest key>=...`, recorded whether or not the crate is
/// ever named. A crate written with `extern crate command_extra;`
/// records the item with the resolver instead, which is what the
/// fixtures under `ui/` do.
///
/// It has blind spots. Because Cargo keys `--extern` by the manifest
/// key, a dependency renamed there
/// (`ce = { package = "command-extra" }`) is not found under
/// `command_extra` -- the caller reads an import of the trait as proof
/// instead, which covers every crate that actually uses it. And
/// `--extern` carries dev-dependencies when the unit being compiled is
/// a test one, so in a crate that depends on `command-extra` only for
/// its tests, `cargo dylint -- --all-targets` opens the gate over the
/// library's own code, where the advice cannot be followed.
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
/// child. A trait reached some other way -- a glob, a project prelude,
/// a `use` inside the body -- reads here as absent, which costs the
/// reader a redundant "add the import" line rather than anything
/// load-bearing.
///
/// The trait is identified by its own name and its crate's, so a crate
/// of the author's own packaged as `command-extra` and exporting a
/// trait called `CommandExtra` reads as the published one. Checking the
/// trait's methods would close that here and nowhere else:
/// [`crate_is_declared`] only ever sees a crate name, which is the
/// point of it -- it answers before anything is loaded. A rule that
/// names a crate cannot tell a crate that took the name.
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
