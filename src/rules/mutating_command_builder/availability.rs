//! Whether the crate whose methods the diagnostic names is reachable
//! from the code under lint.
//!
//! No single answer is the whole of it. Whether the crate under lint
//! declares the dependency, and whether the workspace around it does,
//! are what the `command_extra_dependency` knob chooses between;
//! whether the trait is imported at the call picks which remedy the
//! diagnostic names -- but an import is also evidence of a dependency
//! the declared set cannot see, so the caller reads it for both.
//!
//! Reaching the crate is not the same as having the method. The
//! by-value forms arrived over several releases, so where the trait
//! was loaded it is asked for the one the diagnostic would name.

use crate::cargo_manifest;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, Item, ItemKind, Node};
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// The crate name `command-extra` compiles under, as the compiler
/// spells it rather than as Cargo does.
const CRATE: &str = "command_extra";

/// The trait whose by-value setters the diagnostic names. Where it is
/// not imported, the diagnostic says to import it.
const TRAIT: &str = "CommandExtra";

/// The package name `command-extra` is published under, as Cargo
/// spells it rather than as the compiler does.
const PACKAGE: &str = "command-extra";

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

/// The `CommandExtra` the compilation loaded, where it loaded it at
/// all.
///
/// `None` is not the absence of the dependency -- [`crate_is_declared`]
/// says why a declared crate can be missing from `tcx.crates(())`. It
/// says only that there is no trait here to ask anything of.
pub(super) fn loaded_trait(cx: &LateContext<'_>) -> Option<DefId> {
    let wanted_crate = Symbol::intern(CRATE);
    let wanted_trait = Symbol::intern(TRAIT);
    cx.tcx
        .crates(())
        .iter()
        .filter(|krate| cx.tcx.crate_name(**krate) == wanted_crate)
        .flat_map(|krate| cx.tcx.traits(*krate))
        .find(|def_id| cx.tcx.item_name(**def_id) == wanted_trait)
        .copied()
}

/// Whether `command_extra` -- a [`loaded_trait`] answer -- declares a
/// method named `by_value_form`.
///
/// `CommandExtra` gained its by-value forms over several releases, so
/// which of them exist is a property of the version resolved rather
/// than of the trait. Asking the trait carries no version table to keep
/// in step with the releases, and answers for a method dropped or
/// renamed in some later one as well.
///
/// `is_method` excludes an associated item of the name that no call
/// could reach, as [`super::probe`] does.
pub(super) fn declares_the_counterpart(
    cx: &LateContext<'_>,
    command_extra: DefId,
    by_value_form: &str,
) -> bool {
    cx.tcx
        .associated_items(command_extra)
        .filter_by_name_unhygienic(Symbol::intern(by_value_form))
        .any(rustc_middle::ty::AssocItem::is_method)
}

/// Whether the workspace around the crate under lint declares
/// `command-extra` in `[workspace.dependencies]`.
///
/// None of that table reaches rustc. Cargo resolves inheritance before
/// it assembles the command line, so a dependency the member has not
/// written `command-extra.workspace = true` for leaves no trace in the
/// compiler's own state -- which is the case this answers, and the
/// reason [`crate::cargo_manifest`] reads the file. Where there is no
/// workspace manifest to read, the answer is `false`.
pub(super) fn workspace_declares_the_package() -> bool {
    cargo_manifest::workspace()
        .and_then(|manifest| manifest.get("workspace"))
        .and_then(toml::Value::as_table)
        .is_some_and(|workspace| names_the_package(workspace.get("dependencies")))
}

/// Whether a dependency table holds `command-extra`, under that key or
/// under another key that renames it with `package = "command-extra"`.
fn names_the_package(dependencies: Option<&toml::Value>) -> bool {
    let Some(dependencies) = dependencies.and_then(toml::Value::as_table) else {
        return false;
    };
    dependencies.iter().any(|(key, entry)| {
        key == PACKAGE || entry.get("package").and_then(toml::Value::as_str) == Some(PACKAGE)
    })
}

/// Whether the innermost module around `call` imports `CommandExtra`.
///
/// Scoped to that module because a trait has to be in scope where the
/// method is called, and a parent module's `use` does not reach a
/// child. A trait reached some other way -- a glob, a project prelude,
/// a `use` inside the body -- reads here as absent. What that costs
/// depends on the caller: usually a redundant "add the import" line,
/// but where this answer is the only evidence of the dependency -- as
/// it is under a manifest key that renames the crate -- the lint stays
/// silent instead.
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
