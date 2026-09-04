//! Resolving the canonical module a cherry-picked prelude item
//! actually lives in.
//!
//! A prelude is a module of re-exports, so the path a `use` writes says
//! nothing about where the item is declared. The canonical path is
//! built from the item's *definition* path, which bypasses every
//! re-export on the way and lands on the declaring module.

use rustc_hir::def::{PerNS, Res};
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Ident;
use rustc_span::def_id::{DefId, LOCAL_CRATE};

/// Where a cherry-picked name actually lives.
pub(super) struct Canonical {
    /// The item's canonical path (`crate::thing::A`). `None` when no
    /// single path reproduces the import: the name did not resolve, its
    /// definition path has an unnameable component, or it binds items
    /// in more than one module at once.
    pub(super) path: Option<String>,
    /// Whether every component of every resolved namespace is `pub` up
    /// to the crate root, so the path is nameable from any importing
    /// site. A rewrite onto a path that is not is offered as
    /// `MaybeIncorrect`.
    pub(super) public: bool,
}

/// Resolve the canonical path for the namespaces one `use` path binds.
pub(super) fn resolve(tcx: TyCtxt<'_>, res: PerNS<Option<Res>>) -> Canonical {
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
    let mut paths: Vec<String> = def_ids
        .iter()
        .filter_map(|&def_id| use_path(tcx, def_id))
        .collect();
    paths.sort();
    paths.dedup();
    let [path] = paths.as_slice() else {
        return Canonical {
            path: None,
            public: false,
        };
    };
    Canonical {
        path: Some(path.clone()),
        public: def_ids.iter().all(|&def_id| all_public(tcx, def_id)),
    }
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
