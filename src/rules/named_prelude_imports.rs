//! `perfectionist::named_prelude_imports` — flag cherry-picked named
//! imports from a `prelude` module (`use serde::prelude::Serialize;`),
//! leaving the glob form (`use serde::prelude::*;`) alone.
//!
//! Unlike the source-layout import rules (`import_granularity_mismatch`,
//! `import_grouping_mismatch`, `uncombined_self_import`, `wildcard_imports`), this rule is a
//! plain HIR [`LateLintPass`] rather than a re-parsing one. Two reasons:
//!
//! - Its autofix rewrites the import to the item's *canonical* module,
//!   which is resolved from the item's [`DefId`] — information that only
//!   exists for items the compiler actually resolved, i.e. not for
//!   `#[cfg(...)]`-disabled code a re-parse would additionally reach.
//! - A HIR walk already reaches every compiled module, including
//!   separate-file `mod foo;` submodules; the re-parse machinery exists
//!   to recover *cfg-disabled* and *un-merged* written layout, neither of
//!   which this rule needs. (HIR lowers `use a::{b, c}` into one
//!   [`UseKind::Single`] item per leaf, so each cherry-picked name is
//!   flagged individually with no flattening of our own.)

use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::def::Res;
use rustc_hir::{Item, ItemKind, UseKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_middle::ty::{self, TyCtxt};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::{DefId, LOCAL_CRATE};
use rustc_span::kw;

mod config;

use crate::common::{DefaultState, hir_in_external_macro};
use config::{Config, Resolved};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `use` statement that cherry-picks a named item out of a
    /// `prelude` module (`use serde::prelude::Serialize;`) and leaves the
    /// glob form (`use serde::prelude::*;`) alone. The set of segment
    /// names treated as preludes is configurable via
    /// `prelude_segment_names` (default `["prelude"]`), and individual
    /// prelude paths can be exempted with `allowed_paths`.
    ///
    /// This is the dual of `perfectionist::wildcard_imports`: that rule
    /// restricts globs in general but lets preludes glob freely, while
    /// this rule restricts named imports *from* a prelude and lets the
    /// glob form through.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// `prelude` module is, by convention, a curated set of items the
    /// crate author decided should always travel together as a glob.
    /// Cherry-picking individual items from a prelude defeats that
    /// intent and usually means the importer should reach into the
    /// prelude's source module instead. A standalone import is rewritten
    /// to the item's canonical module; a brace-list leaf
    /// (`use foo::prelude::{A, B};`) — or a name that resolves through
    /// several modules at once — is flagged with a `help` instead, since
    /// a single `use` can't always reproduce it.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// use serde::prelude::Serialize;
    /// use diesel::prelude::{table, AsChangeset};
    /// ```
    ///
    /// **Prefer:**
    ///
    /// _From the canonical module:_ import each item where it actually lives.
    ///
    /// ```rust,ignore
    /// use serde::Serialize;
    /// use diesel::{table, AsChangeset};
    /// ```
    ///
    /// _As a prelude glob:_ pull in the whole curated set at once.
    ///
    /// ```rust,ignore
    /// use diesel::prelude::*;
    /// ```
    pub perfectionist::NAMED_PRELUDE_IMPORTS,
    Warn,
    "named item cherry-picked from a prelude module instead of glob-imported",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::named_prelude_imports";

pub struct NamedPreludeImports {
    config: Resolved,
}

impl_lint_pass!(NamedPreludeImports => [NAMED_PRELUDE_IMPORTS]);

impl Register for rule::NamedPreludeImports {
    /// Active by default. The prelude convention is the shipped
    /// baseline; `prelude_segment_names` / `allowed_paths` tune it.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[NAMED_PRELUDE_IMPORTS]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Every `allowed_paths` entry has to end with a prelude segment (it
        // matches the path up to and including the prelude), so reject a
        // misconfigured one loudly rather than letting it silently match
        // nothing.
        config::validate(&config).unwrap_or_else(|message| {
            panic!("perfectionist::named_prelude_imports: {message}");
        });
        lint_store.register_late_lint_pass(Box::new(move |_| {
            Box::new(NamedPreludeImports {
                config: Resolved::from_config(config.clone()),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for NamedPreludeImports {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only single (named) imports are flagged; the glob and the
        // synthetic `ListStem` forms are left alone.
        let ItemKind::Use(path, UseKind::Single(_)) = &item.kind else {
            return;
        };
        if item.span.from_expansion() || hir_in_external_macro(cx, item.hir_id(), item.span) {
            return;
        }

        let segments = path.segments;
        // The last segment is the imported item itself; a prelude segment
        // must sit *before* it (something is cherry-picked from under the
        // prelude). `use serde::prelude;` — the prelude as the leaf — is
        // not a cherry-pick and is left alone.
        let Some(item_split) = segments.len().checked_sub(1) else {
            return;
        };
        let Some(prelude_index) = segments[..item_split].iter().position(|segment| {
            self.config
                .prelude_segment_names
                .contains(segment.ident.name.as_str())
        }) else {
            return;
        };

        // `allowed_paths` entries are absolute (e.g. `crate::prelude` for a
        // crate-root path, `::serde::prelude` for an extern crate).
        // `join_segments` drops any `PathRoot`, so both `use crate::prelude::Item`
        // and a `::`-rooted form arrive identically; `canonical_key` forms
        // the absolute key matched against the allow list. The key is the
        // module path up to and including the prelude segment.
        let prelude_path =
            crate::abs_path::canonical_key(&join_segments(&segments[..=prelude_index]));
        if self.config.allowed_paths.contains(&prelude_path) {
            return;
        }

        let written_path = join_segments(segments);
        let fix = canonical_fix(cx, path.res, path.span, &written_path);
        span_lint_hir_and_then(
            cx,
            NAMED_PRELUDE_IMPORTS,
            item.hir_id(),
            path.span,
            "named item cherry-picked from a prelude module",
            |diagnostic| {
                if let Some((replacement, applicability)) = &fix {
                    diagnostic.span_suggestion(
                        path.span,
                        "import the item from its canonical module",
                        replacement,
                        *applicability,
                    );
                } else {
                    diagnostic.help(
                        "import this item from its canonical module, or glob-import the \
                         prelude with `use ...::prelude::*;`",
                    );
                }
            },
        );
    }
}

/// The dotted-path string of a run of path segments
/// (`["serde", "prelude", "Serialize"]` → `"serde::prelude::Serialize"`).
/// A leading `::` shows up in the HIR path as a synthetic `PathRoot`
/// segment; skip it (as `wildcard_imports::collect_globs` and
/// `uncombined_self_import::real_segments` do) so a `use ::serde::prelude::Item;`
/// normalises to `serde::prelude::Item` for `allowed_paths` matching
/// rather than `{{root}}::serde::prelude::Item`.
fn join_segments(segments: &[rustc_hir::PathSegment<'_>]) -> String {
    segments
        .iter()
        .filter(|segment| segment.ident.name != kw::PathRoot)
        .map(|segment| segment.ident.name.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Build the `(replacement, applicability)` for rewriting the import to
/// the item's canonical module, or `None` when no mechanical fix is
/// offered: the item didn't resolve, its definition path has an
/// unnameable component, or the written path span doesn't match the
/// reconstructed `written_path` (an awkward brace-list leaf whose span
/// covers only part of the source path). The un-fixable leaves are still
/// flagged — they just carry a `help` rather than a suggestion.
fn canonical_fix(
    cx: &LateContext<'_>,
    res: rustc_hir::def::PerNS<Option<Res>>,
    path_span: rustc_span::Span,
    written_path: &str,
) -> Option<(String, Applicability)> {
    // A `use` brings in *every* namespace the name resolves to. Collect a
    // DefId per resolved namespace (type / value / macro), not just the
    // first: a name bound in two namespaces can resolve to items in
    // *different* modules, and a single rewritten `use` cannot reproduce
    // that.
    let def_ids: Vec<DefId> = [res.type_ns, res.value_ns, res.macro_ns]
        .into_iter()
        .flatten()
        .filter_map(|res| res.opt_def_id())
        .collect();

    // The distinct nameable canonical paths the name resolves to. An
    // unnameable component (a tuple/unit struct's value-namespace
    // constructor, an `impl`, and so on) drops out via
    // `canonical_use_path`'s `None`, which is what leaves a unit struct —
    // type plus constructor — with the single struct path. Only offer a
    // mechanical fix when every resolved namespace agrees on one canonical
    // path; more than one means the import spans several modules and no
    // single `use` reproduces it, so it stays flagged with a `help`
    // instead of a wrong machine-applicable rewrite.
    let mut canonicals: Vec<String> = def_ids
        .iter()
        .filter_map(|&def_id| canonical_use_path(cx.tcx, def_id))
        .collect();
    canonicals.sort();
    canonicals.dedup();
    let [canonical] = canonicals.as_slice() else {
        return None;
    };

    // Only offer the suggestion when the written path span snippet is
    // exactly the path we'd reconstruct from its segments — a
    // self-contained `serde::prelude::Serialize`, not a brace-list leaf
    // whose span covers only part of the source path. Replacing the
    // whole span then stays well-formed and preserves any `as` rename
    // and trailing `;` that sit outside the path span.
    let written = cx.sess().source_map().span_to_snippet(path_span).ok()?;
    if written != written_path {
        return None;
    }

    // The canonical path is publicly nameable when every component of
    // every resolved namespace is `pub` up to the crate root; otherwise a
    // definition module may be private and the rewrite needs a human's eye.
    let applicability = if def_ids.iter().all(|&def_id| all_public(cx.tcx, def_id)) {
        Applicability::MachineApplicable
    } else {
        Applicability::MaybeIncorrect
    };
    Some((canonical.clone(), applicability))
}

/// The canonical `use`-able path for a [`DefId`]: the crate (`crate` for
/// the local crate, else the crate's name) followed by each named
/// component of the item's *definition* path. Returns `None` if any
/// component has no nameable identifier (an `impl`, a closure, etc.), which
/// means the item can't be addressed by a plain path.
fn canonical_use_path(tcx: TyCtxt<'_>, def_id: DefId) -> Option<String> {
    let def_path = tcx.def_path(def_id);
    let mut segments = Vec::with_capacity(def_path.data.len() + 1);
    if def_path.krate == LOCAL_CRATE {
        segments.push("crate".to_owned());
    } else {
        segments.push(tcx.crate_name(def_path.krate).to_string());
    }
    for component in &def_path.data {
        // Render each name through an `Ident` so a keyword module name
        // round-trips as a raw identifier (`r#type`, not the bare keyword
        // `type`); a plain `Symbol::to_string()` drops the `r#` and the
        // suggested path would fail to parse. Mirrors
        // `uncombined_self_import::render_segments`.
        let name = component.data.get_opt_name()?;
        segments.push(rustc_span::Ident::with_dummy_span(name).to_string());
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
