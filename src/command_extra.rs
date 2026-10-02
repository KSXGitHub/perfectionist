//! `command-extra`'s trait as the rules that name it have to ask about
//! it: under the name the compiler spells, and at the call site whose
//! imports decide whether a suggestion naming one of its methods
//! resolves.
//!
//! Every rule that asks names a trait method rather than resolving one,
//! so each needs the same answer about scope, and the reasoning behind
//! that answer lives here once.

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, Item, ItemKind, Node};
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// The crate name `command-extra` compiles under, as the compiler
/// spells it rather than as Cargo does.
pub(crate) const CRATE: &str = "command_extra";

/// The name `command-extra` declares its extension trait under.
pub(crate) const TRAIT: &str = "CommandExtra";

/// Whether `trait_id` is `command_extra::CommandExtra`.
///
/// The name alone would match a `CommandExtra` of the author's own, so
/// the crate is asked for too. That still reads a crate of the author's
/// own, packaged as `command-extra` and exporting a trait called
/// `CommandExtra`, as the published one. Checking the trait's methods
/// would close that here and nowhere else: `crate_is_declared`, in
/// `src/rules/mutating_command_builder/availability.rs`, only ever sees
/// a crate name, because it answers before anything is loaded.
pub(crate) fn is_the_trait(cx: &LateContext<'_>, trait_id: DefId) -> bool {
    cx.tcx.item_name(trait_id) == Symbol::intern(TRAIT)
        && cx.tcx.crate_name(trait_id.krate) == Symbol::intern(CRATE)
}

/// Whether the innermost module around `call` imports `CommandExtra`,
/// as [`is_the_trait`] identifies it. A rule that has resolved the trait
/// asks [`imports`] about that trait instead.
pub(crate) fn trait_is_imported(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    imports(cx, call, |def_id| is_the_trait(cx, def_id))
}

/// Whether the innermost module around `call` imports a trait `wanted`
/// accepts.
///
/// Scoped to that module because a trait has to be in scope where the
/// method is called, and a parent module's `use` does not reach a
/// child. A trait reached some other way, such as through a glob, a
/// project prelude or a `use` inside the body, reads here as absent.
/// What that costs depends on the caller: usually a withheld fix and a
/// redundant "add the import" line, but where this answer is the only
/// evidence of the dependency, as it is under a manifest key that
/// renames the crate, the caller's lint stays silent instead.
pub(crate) fn imports(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    wanted: impl Fn(DefId) -> bool,
) -> bool {
    let module = cx.tcx.parent_module(call.hir_id);
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
        .any(|res| matches!(res, Some(Res::Def(DefKind::Trait, def_id)) if wanted(*def_id)))
}
