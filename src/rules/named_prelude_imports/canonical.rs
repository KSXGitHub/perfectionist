//! Resolving the canonical module a cherry-picked prelude item
//! actually lives in.
//!
//! A prelude is a module of re-exports, so the path a `use` writes says
//! nothing about where the item is declared. The canonical path is
//! built from the item's *definition* path, which bypasses every
//! re-export on the way and lands on the declaring module.
//!
//! A definition path is not always a path the importer may *write*.
//! Two ways it is not, each of which produced a rewrite that does not
//! compile before this module weighed them — and which are answered
//! differently, because they are wrong to different depths:
//!
//! - **The defining crate need not be linked here.** `std`'s prelude
//!   re-exports items defined in `alloc`, so `use std::prelude::v1::Vec;`
//!   has the definition path `alloc::vec::Vec` — and `alloc` is not in
//!   the extern prelude of an ordinary crate. Any facade crate
//!   re-exporting from a private sibling behaves the same way. The
//!   module named is still the right one, so the path is offered and
//!   only [`Canonical::nameable`] is withheld.
//! - **A macro's definition path need not be the path it answers to.** A
//!   `#[macro_export] macro_rules!` written inside `mod thing` has the
//!   definition path `crate::thing::shout`, but resolves only as
//!   `crate::shout`. Here the module named is simply wrong, so no path
//!   is offered at all rather than a suggestion that reads as fact.

use rustc_hir::def::{DefKind, PerNS, Res};
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Ident;
use rustc_span::def_id::{DefId, LOCAL_CRATE};

/// Where a cherry-picked name actually lives.
pub(super) struct Canonical {
    /// The item's canonical path (`crate::thing::A`). `None` when no
    /// single path reproduces the import: the name did not resolve, its
    /// definition path has an unnameable component, it binds items in
    /// more than one module at once, or it is a macro, whose definition
    /// path need not be the path it answers to (see the module docs).
    pub(super) path: Option<String>,
    /// Whether the canonical path is one the importer can be promised
    /// resolves, which is what separates a `MachineApplicable` rewrite
    /// from a `MaybeIncorrect` one. It certifies both of:
    ///
    /// - every component of every resolved namespace is `pub` up to the
    ///   crate root, so the path is nameable from any importing site;
    /// - the path is rooted in a crate the `use` being rewritten already
    ///   names, so that crate is linked here (see the module docs).
    ///
    /// Each is a promise the rule can keep. A path failing either is
    /// still offered — it names the right module — just without the
    /// promise that applying it compiles.
    pub(super) nameable: bool,
}

/// Resolve the canonical path for the namespaces one `use` path binds.
/// `written_root` is the first segment of the path being rewritten
/// (`std` of `use std::prelude::v1::Vec;`), which says which crates the
/// importer is known to have linked.
pub(super) fn resolve(
    tcx: TyCtxt<'_>,
    res: PerNS<Option<Res>>,
    written_root: Option<&str>,
) -> Canonical {
    // A `use` brings in *every* namespace the name resolves to. Collect
    // a `DefId` per resolved namespace (type / value / macro), not just
    // the first: a name bound in two namespaces can resolve to items in
    // *different* modules, and a single rewritten `use` cannot reproduce
    // that.
    let def_ids: Vec<DefId> = [res.type_ns, res.value_ns, res.macro_ns]
        .into_iter()
        .flatten()
        .filter_map(|res| res.opt_def_id())
        .collect();

    // The distinct nameable canonical paths the name resolves to. An
    // unnameable component (a tuple/unit struct's value-namespace
    // constructor, an `impl`, and so on) drops out via [`use_path`]'s
    // `None`, which is what leaves a unit struct — type plus constructor
    // — with the single struct path. Exactly one path means every
    // resolved namespace agrees; more than one means the import spans
    // several modules and no single `use` reproduces it, so no rewrite
    // is offered rather than a wrong one.
    // A macro is dropped rather than resolved: `#[macro_export]` lifts
    // it to its crate root, so its definition path names a module it
    // cannot be reached through. Dropping it leaves `paths` empty, which
    // withholds the suggestion for the whole statement.
    if def_ids
        .iter()
        .any(|&def_id| matches!(tcx.def_kind(def_id), DefKind::Macro(_)))
    {
        return Canonical {
            path: None,
            nameable: false,
        };
    }
    let mut paths: Vec<String> = def_ids
        .iter()
        .filter_map(|&def_id| use_path(tcx, def_id))
        .collect();
    paths.sort();
    paths.dedup();
    let [path] = paths.as_slice() else {
        return Canonical {
            path: None,
            nameable: false,
        };
    };
    let nameable = crate_is_linked(path, written_root)
        && def_ids.iter().all(|&def_id| all_public(tcx, def_id));
    Canonical {
        path: Some(path.clone()),
        nameable,
    }
}

/// Whether `canonical`'s crate root is one the importer can name: the
/// local crate, which `crate::` always reaches, or the very crate the
/// rewritten `use` already spells, which is therefore linked here.
///
/// Anything else is a crate this compilation can see only because
/// something else depends on it — reachable through the re-export the
/// importer wrote, but not necessarily by name.
fn crate_is_linked(canonical: &str, written_root: Option<&str>) -> bool {
    let root = canonical.split("::").next().unwrap_or(canonical);
    root == "crate" || written_root == Some(root)
}

/// The canonical `use`-able path for a [`DefId`]: the crate (`crate`
/// for the local crate, else the crate's name) followed by each named
/// component of the item's *definition* path. Returns `None` if any
/// component has no nameable identifier (an `impl`, a closure, etc.),
/// which means the item can't be addressed by a plain path.
fn use_path(tcx: TyCtxt<'_>, def_id: DefId) -> Option<String> {
    let def_path = tcx.def_path(def_id);
    let mut segments = Vec::with_capacity(def_path.data.len() + 1);
    if def_path.krate == LOCAL_CRATE {
        segments.push("crate".to_owned());
    } else {
        segments.push(tcx.crate_name(def_path.krate).to_string());
    }
    for component in &def_path.data {
        // Render each name through an `Ident` so a keyword module name
        // round-trips as a raw identifier (`r#type`, not the bare
        // keyword `type`); a plain `Symbol::to_string()` drops the `r#`
        // and the suggested path would fail to parse. Mirrors
        // `uncombined_self_import::render_segments`.
        let name = component.data.get_opt_name()?;
        segments.push(Ident::with_dummy_span(name).to_string());
    }
    Some(segments.join("::"))
}

/// Whether `def_id` and every enclosing module up to the crate root is
/// `pub`, so the canonical path is nameable from any importing site.
fn all_public(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    let mut current = def_id;
    loop {
        if !matches!(tcx.visibility(current), ty::Visibility::Public) {
            return false;
        }
        match tcx.opt_parent(current) {
            Some(parent) => current = parent,
            None => return true,
        }
    }
}

#[cfg(test)]
mod tests;
