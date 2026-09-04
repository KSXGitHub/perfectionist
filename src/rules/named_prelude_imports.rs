//! `perfectionist::named_prelude_imports` — flag cherry-picked named
//! imports from a `prelude` module (`use serde::prelude::Serialize;`),
//! leaving the glob form (`use serde::prelude::*;`) alone.
//!
//! Unlike the source-layout import rules (`import_granularity_mismatch`,
//! `import_grouping_mismatch`, `uncombined_self_import`, `wildcard_imports`), this rule is a
//! plain HIR [`LateLintPass`] rather than a re-parsing one. Two reasons:
//!
//! - Its autofix rewrites the import to the item's *canonical* module,
//!   which is resolved from the item's `DefId` — information that only
//!   exists for items the compiler actually resolved, i.e. not for
//!   `#[cfg(...)]`-disabled code a re-parse would additionally reach.
//! - A HIR walk already reaches every compiled module, including
//!   separate-file `mod foo;` submodules; the re-parse machinery exists
//!   to recover *cfg-disabled* and *un-merged* written layout, neither of
//!   which this rule needs. (HIR lowers `use a::{b, c}` into one
//!   [`UseKind::Single`] item per leaf, so each cherry-picked name is
//!   flagged individually with no flattening of our own.)
//!
//! Lowering does drop one thing the fix needs: which leaves were
//! written as one statement. A brace-list leaf's path span holds only
//! its name, so the rewrite has to rebuild the statement's whole use
//! tree (see [`statement`]). The desugaring emits a statement's
//! `ListStem` ahead of its leaves and nests every leaf span inside the
//! stem's, so the pass rebuilds that grouping as it walks: a `use` item
//! the current statement's span does not contain starts the next one.

use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::def::{PerNS, Res};
use rustc_hir::{Item, ItemKind, PathSegment, UseKind, UsePath};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::kw;

mod canonical;
mod config;
mod statement;

use crate::common::{DefaultState, hir_in_external_macro, join_path_segments};
use config::{Config, Resolved};
use statement::{Fix, Leaf, Statement};

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
    /// prelude's source module instead.
    ///
    /// Each cherry-picked name is flagged on its own, and the rewrite
    /// re-points the whole `use` onto the canonical modules at once —
    /// so it is offered once per statement, on the first name it
    /// covers, and the rest carry a `help`. Where the entries end up
    /// sharing a prefix the rewrite keeps them in one statement
    /// (`diesel::prelude::{table, AsChangeset}` becomes
    /// `diesel::{table, AsChangeset}`); grouping them any differently is
    /// `perfectionist::import_granularity_mismatch`'s business.
    ///
    /// These shapes get a `help` instead of a rewrite:
    ///
    /// - A name that resolves to items in several modules at once,
    ///   which no single `use` reproduces.
    /// - A statement holding a `self` entry
    ///   (`use foo::prelude::{self, Bar};`), which would stop binding
    ///   only the module once the tree is rebuilt around it.
    /// - A macro, which `#[macro_export]` reaches at its crate root
    ///   rather than through the module it is written in.
    ///
    /// A rewrite onto a module in some *third* crate — `std`'s prelude
    /// re-exports items that live in `alloc` — names the right module
    /// but a crate this file has not necessarily linked, so it is
    /// offered for you to check rather than applied by
    /// `cargo dylint --fix`.
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

/// The one diagnostic the rule emits, once per cherry-picked name.
const MESSAGE: &str = "named item cherry-picked from a prelude module";

/// Fallback for a leaf no mechanical rewrite is offered for.
const HELP: &str = "import this item from its canonical module, or glob-import the prelude with \
                    `use ...::prelude::*;`";

pub struct NamedPreludeImports {
    config: Resolved,
    /// The `use` statement being walked, held until the pass reaches an
    /// item outside it. A brace list's fix rewrites the statement as a
    /// whole, so no leaf can be judged until every sibling leaf is in.
    statement: Option<Statement>,
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
                statement: None,
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for NamedPreludeImports {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Use(path, kind) = &item.kind else {
            return;
        };
        if item.span.from_expansion() || hir_in_external_macro(cx, item.hir_id(), item.span) {
            self.flush(cx);
            return;
        }
        let leaf = self.leaf(cx, item, path, *kind);
        // A statement's leaves are lowered nested inside its head item's
        // span, so an item that head does not contain opens the next
        // statement.
        if !self
            .statement
            .as_ref()
            .is_some_and(|statement| statement.span.contains(item.span))
        {
            self.flush(cx);
            self.statement = Some(Statement::new(item, path.span, *kind));
        }
        if let (Some(statement), Some(leaf)) = (self.statement.as_mut(), leaf) {
            statement.leaves.push(leaf);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        self.flush(cx);
    }
}

impl NamedPreludeImports {
    /// Read one lowered `use` item as a leaf of the statement being
    /// walked. `None` for a `ListStem`, which is the statement's head
    /// rather than anything it imports.
    fn leaf(
        &self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        path: &UsePath<'_>,
        kind: UseKind,
    ) -> Option<Leaf> {
        let segments = path.segments;
        let rename = match kind {
            UseKind::Single(binding) => statement::rename_suffix(segments, binding),
            UseKind::Glob => String::new(),
            UseKind::ListStem => return None,
        };
        let mut written = statement::written_path(segments);
        if matches!(kind, UseKind::Glob) && !written.is_empty() {
            written.push_str("::*");
        }
        Some(Leaf {
            hir_id: item.hir_id(),
            path_span: path.span,
            is_self: cx
                .sess()
                .source_map()
                .span_to_snippet(path.span)
                .is_ok_and(|source| statement::is_self_entry(&source)),
            flagged: matches!(kind, UseKind::Single(_))
                .then(|| self.cherry_pick(cx, segments, path.res))
                .flatten(),
            written,
            rename,
        })
    }

    /// Where a leaf's item canonically lives, when the leaf cherry-picks
    /// it out of a prelude; `None` when the path names no prelude, or
    /// names one the configuration exempts.
    fn cherry_pick(
        &self,
        cx: &LateContext<'_>,
        segments: &[PathSegment<'_>],
        res: PerNS<Option<Res>>,
    ) -> Option<canonical::Canonical> {
        // The last segment is the imported item itself; a prelude segment
        // must sit *before* it (something is cherry-picked from under the
        // prelude). `use serde::prelude;` — the prelude as the leaf — is
        // not a cherry-pick and is left alone.
        let item_split = segments.len().checked_sub(1)?;
        let prelude_index = segments[..item_split].iter().position(|segment| {
            self.config
                .prelude_segment_names
                .contains(segment.ident.name.as_str())
        })?;

        // `allowed_paths` entries are absolute (e.g. `crate::prelude` for a
        // crate-root path, `::serde::prelude` for an extern crate).
        // `join_path_segments` drops any `PathRoot`, so both `use crate::prelude::Item`
        // and a `::`-rooted form arrive identically; `canonical_key` forms
        // the absolute key matched against the allow list. The key is the
        // module path up to and including the prelude segment.
        let prelude_path =
            crate::abs_path::canonical_key(&join_path_segments(&segments[..=prelude_index]));
        if self.config.allowed_paths.contains(&prelude_path) {
            return None;
        }
        // The written path's own root says which crates this site is
        // known to have linked, which is half of whether the canonical
        // path can be promised to resolve. `PathRoot` is the leading
        // `::` of `use ::serde::...`, not a segment of its own.
        let written_root = segments
            .iter()
            .map(|segment| segment.ident.name)
            .find(|name| *name != kw::PathRoot);
        Some(canonical::resolve(
            cx.tcx,
            res,
            written_root.as_ref().map(rustc_span::Symbol::as_str),
        ))
    }

    /// Emit for the statement just walked past, if any of its leaves
    /// cherry-picked from a prelude.
    fn flush(&mut self, cx: &LateContext<'_>) {
        let Some(statement) = self.statement.take() else {
            return;
        };
        let flagged: Vec<&Leaf> = statement
            .leaves
            .iter()
            .filter(|leaf| leaf.flagged.is_some())
            .collect();
        let Some((&first, rest)) = flagged.split_first() else {
            return;
        };

        // Only one rewrite is offered per statement: it replaces the
        // statement's whole use tree, so it already covers every
        // cherry-pick in it. It rides the first of them; the rest carry
        // the fallback help.
        emit(cx, first, statement.rewrite(cx));
        for &leaf in rest {
            emit(cx, leaf, None);
        }
    }
}

/// A rewrite onto a canonical path every importer can name is
/// mechanical; one onto a path that is private somewhere up to the
/// crate root needs a human's eye.
fn applicability(nameable: bool) -> Applicability {
    if nameable {
        Applicability::MachineApplicable
    } else {
        Applicability::MaybeIncorrect
    }
}

/// Emit one leaf's diagnostic, carrying `fix` when a mechanical rewrite
/// is offered for it and the fallback [`HELP`] otherwise.
fn emit(cx: &LateContext<'_>, leaf: &Leaf, fix: Option<Fix>) {
    span_lint_hir_and_then(
        cx,
        NAMED_PRELUDE_IMPORTS,
        leaf.hir_id,
        leaf.path_span,
        MESSAGE,
        |diagnostic| {
            if let Some(fix) = fix {
                diagnostic.span_suggestion(fix.span, fix.label, fix.replacement, fix.applicability);
            } else {
                diagnostic.help(HELP);
            }
        },
    );
}
