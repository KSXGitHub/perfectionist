use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::def::{DefKind, Namespace, Res};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::{CRATE_DEF_ID, DefId, LocalDefId};
use rustc_span::{Span, Symbol};

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolve_symbol_set, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;

mod casing;
mod scan;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a backticked identifier in a doc comment (`` `Foo` ``)
    /// that resolves as a Rust path in the documented item's scope but
    /// is not written as a rustdoc intra-doc link (`` [`Foo`] ``).
    ///
    /// Only bare single identifiers whose name resolves to an item in
    /// the enclosing module's scope are flagged; a backticked word that
    /// names nothing in scope is left alone.
    ///
    /// A publicly-reachable item that mentions a *private* (not
    /// publicly-reachable) item is also left alone: turning that mention
    /// into a link would make rustdoc's `rustdoc::private_intra_doc_links`
    /// fire under a plain `cargo doc`, and a public item leaning on a
    /// private one is a separate concern from this rule's.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Both
    /// `` `Foo` `` and `` [`Foo`] `` render as monospaced text, so the
    /// page looks the same at a glance. The link form additionally
    /// turns the mention into a clickable cross-reference and lets
    /// rustdoc's `rustdoc::broken_intra_doc_links` lint catch the day a
    /// rename leaves the prose pointing at a type that no longer
    /// exists. Spelling every in-scope mention as a link keeps the
    /// documentation navigable and the references checked.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// /// Installs the package described by `PackageManifest` into `Store`.
    /// pub fn install(manifest: &PackageManifest, store: &Store) {}
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// /// Installs the package described by [`PackageManifest`] into [`Store`].
    /// pub fn install(manifest: &PackageManifest, store: &Store) {}
    /// ```
    pub perfectionist::INTRA_DOC_LINKS,
    Warn,
    "backticked identifier in a doc comment that resolves in scope should be an intra-doc link",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::intra_doc_links";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Identifiers the rule never suggests linking, even when they
    /// resolve in scope. Empty by default. Use this for a name a doc
    /// comment deliberately mentions without wanting a cross-reference
    /// — a historical type kept for context, or a word that happens to
    /// collide with an in-scope item but is meant as prose.
    skip_idents: Vec<String>,
    /// How far from the documenting item a referenced name may resolve
    /// for the rule to check it, by where the referenced item lives in
    /// the module tree. A backticked word that matches an
    /// accidentally-added cross-module import is a common source of
    /// churn, so a project can narrow this. Defaults to `anywhere`.
    reference_scope: ReferenceScope,
    /// Whether to check `PascalCase` names. Defaults to `true`.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this field: such a spelling
    /// is rare and, when it matches a local identifier, rarely an
    /// accident.
    check_pascal_case: bool,
    /// Whether to check `UPPER_CASE` (`SCREAMING_SNAKE_CASE`) names.
    /// Defaults to `true`.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this field: such a spelling
    /// is rare and, when it matches a local identifier, rarely an
    /// accident.
    check_upper_case: bool,
    /// Whether to check `snake_case` names. Defaults to `true`.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this field: such a spelling
    /// is rare and, when it matches a local identifier, rarely an
    /// accident.
    check_snake_case: bool,
    /// Minimum number of words a name must have to be checked. Defaults
    /// to `1` (check everything). At `3`, `foo`, `foo_bar`, `Foo`,
    /// `FooBar`, `FOO`, and `FOO_BAR` are exempt, while `foo_bar_baz`,
    /// `FooBarBaz`, and `FOO_BAR_BAZ` are checked.
    ///
    /// A mixed or non-conformist name (`fooBar`, `foo_BAR`, `__foo`,
    /// `foo__bar`) is checked regardless of this threshold: such a
    /// spelling is rare and, when it matches a local identifier, rarely
    /// an accident.
    min_words: NonZeroUsize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            skip_idents: Vec::new(),
            reference_scope: ReferenceScope::default(),
            check_pascal_case: true,
            check_upper_case: true,
            check_snake_case: true,
            min_words: NonZeroUsize::MIN,
        }
    }
}

/// How far from the documenting item a referenced name may resolve for
/// the rule to check it, configured by `reference_scope`. The axis is
/// *where the referenced item lives relative to the documenting item's
/// module*, not the spelling of the `use` path that brought it in.
#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReferenceScope {
    /// Only items defined directly in the documenting item's own module.
    /// Every name reached through a `use` is left alone.
    OwnModule,
    /// Also items from within that module's own subtree (e.g. reached
    /// through `use self::child::Item`), but still not names that reach
    /// outside the module — `use super::...`, `use crate::...`, and
    /// imports from other crates.
    ModuleTree,
    /// Any name that resolves in scope, however it got there.
    #[default]
    Anywhere,
}

pub struct IntraDocLinks {
    skip_idents: BTreeSet<Symbol>,
    reference_scope: ReferenceScope,
    check_pascal_case: bool,
    check_upper_case: bool,
    check_snake_case: bool,
    min_words: usize,
}

impl IntraDocLinks {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let skip_idents = resolve_symbol_set(&[], config.skip_idents, Vec::new());
        Self {
            skip_idents,
            reference_scope: config.reference_scope,
            check_pascal_case: config.check_pascal_case,
            check_upper_case: config.check_upper_case,
            check_snake_case: config.check_snake_case,
            min_words: config.min_words.get(),
        }
    }

    /// Whether the candidate's name passes the case and minimum-word-count
    /// knobs. A mixed / non-conformist name is always checked — those
    /// knobs only gate the three conformist shapes — so it short-circuits
    /// to `true`.
    fn name_allows(&self, ident: &str) -> bool {
        let case = casing::classify(ident);
        let case_enabled = match case {
            casing::Case::Snake => self.check_snake_case,
            casing::Case::Upper => self.check_upper_case,
            casing::Case::Pascal => self.check_pascal_case,
            casing::Case::NonConformist => return true,
        };
        case_enabled && casing::word_count(ident, case) >= self.min_words
    }
}

impl_lint_pass!(IntraDocLinks => [INTRA_DOC_LINKS]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[INTRA_DOC_LINKS]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("intra_doc_links", DefaultState::Active) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(IntraDocLinks::new()));
}

/// One parked finding: the identifier text plus the source snippet of
/// the whole `` `Foo` `` code span, used to build the autofix.
struct Violation {
    ident: Symbol,
    snippet: String,
}

impl<'tcx> LateLintPass<'tcx> for IntraDocLinks {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let mut violations: Vec<(Span, Violation)> = Vec::new();
        walk_local_comments(lint_context, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                self.collect_doc_chunk(lint_context, chunk, &mut violations);
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {}
        });
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, violation| {
            // Resolution is deferred to here: `emit_at_enclosing_hir`
            // has just told us which HIR node the doc comment documents,
            // which is the scope a rustdoc intra-doc link resolves in.
            if let Some(resolution) = self.resolve_in_scope(lint_context, hir_id, violation.ident) {
                emit(lint_context, hir_id, span, &violation, resolution);
            }
        });
    }
}

impl IntraDocLinks {
    fn collect_doc_chunk(
        &self,
        lint_context: &LateContext<'_>,
        chunk: &CommentChunk<'_>,
        out: &mut Vec<(Span, Violation)>,
    ) {
        for candidate in scan::collect_candidates(&chunk.rendered) {
            let name = Symbol::intern(&candidate.ident);
            if self.skip_idents.contains(&name) {
                continue;
            }
            if !self.name_allows(&candidate.ident) {
                continue;
            }
            let len = (candidate.span.end - candidate.span.start) as u32;
            let Some(span) = chunk.span_for(candidate.span.start, len) else {
                continue;
            };
            // Prefer the real source text for the autofix so the
            // backtick run and any padding are preserved verbatim; fall
            // back to the rendered slice if the span isn't snippet-able.
            let snippet = lint_context
                .sess()
                .source_map()
                .span_to_snippet(span)
                .unwrap_or_else(|_| chunk.rendered[candidate.span.clone()].to_owned());
            out.push((
                span,
                Violation {
                    ident: name,
                    snippet,
                },
            ));
        }
    }
}

/// How a candidate identifier resolves in the documented item's scope.
#[derive(Clone, Copy)]
enum Resolution {
    /// Resolves in exactly one namespace — a plain `` [`Foo`] `` link
    /// is unambiguous, so the autofix is machine-applicable.
    Unique,
    /// The name exists in more than one namespace (e.g. a type and a
    /// function). A bare `` [`Foo`] `` would be an ambiguous intra-doc
    /// link, so the rule emits a help note rather than an autofix.
    Ambiguous,
}

impl IntraDocLinks {
    /// Resolve `name` against the children of the documented item's
    /// scope module. Returns `None` when the name names nothing in scope
    /// the rule still checks (so the backticks are deliberate prose, not
    /// an unlinked reference).
    ///
    /// Two filters drop a candidate before it counts:
    ///
    /// - The `reference_scope` policy (see [`ReferenceScope`]) drops a
    ///   name whose target lives farther out than the project allows.
    /// - A publicly-reachable item is never linked to a private (not
    ///   publicly-reachable) target: turning `` `Priv` `` into
    ///   `` [`Priv`] `` in the docs of a `pub` item would make rustdoc's
    ///   `rustdoc::private_intra_doc_links` fire under a plain
    ///   `cargo doc`, and a public item that leans on a private one is
    ///   questionable to begin with. Flagging that (or "fixing" it) is
    ///   the job of whatever rule governs public-references-private, not
    ///   this one.
    fn resolve_in_scope(
        &self,
        cx: &LateContext<'_>,
        hir_id: hir::HirId,
        name: Symbol,
    ) -> Option<Resolution> {
        let scope = scope_module(cx, hir_id);
        let effective_visibilities = cx.tcx.effective_visibilities(());
        let documented_public = effective_visibilities.is_reachable(documented_def_id(cx, hir_id));
        // One slot per namespace (`TypeNS`, `ValueNS`, `MacroNS`); a name
        // present in more than one is an ambiguous intra-doc link.
        let mut namespaces = [false; 3];
        let mut found = false;
        for child in cx.tcx.module_children_local(scope) {
            if child.ident.name != name {
                continue;
            }
            let target = child.res.opt_def_id();
            // Imported-name policy: drop a candidate the project chose to
            // ignore based on where its target lives relative to `scope`.
            let reach = target.map_or(Reach::Outside, |def_id| import_reach(cx, scope, def_id));
            let ignored_by_policy = match self.reference_scope {
                ReferenceScope::OwnModule => reach != Reach::LocalDefinition,
                ReferenceScope::ModuleTree => reach == Reach::Outside,
                ReferenceScope::Anywhere => false,
            };
            if ignored_by_policy {
                continue;
            }
            // Public-references-private exemption (see the doc comment): a
            // target is "private" when it is a local item that is not
            // publicly reachable. An external target (e.g. a re-exported
            // `std` type) is public, so it never triggers the exemption.
            let target_private = target
                .and_then(|def_id| def_id.as_local())
                .is_some_and(|local| !effective_visibilities.is_reachable(local));
            if documented_public && target_private {
                continue;
            }
            found = true;
            // A unit/tuple struct (or enum variant) introduces a value-ns
            // constructor that shares the type's identity; rustdoc resolves
            // `` [`Foo`] `` to the type without complaint, so the
            // constructor must not count toward namespace ambiguity.
            if matches!(child.res, Res::Def(DefKind::Ctor(..), _)) {
                continue;
            }
            match child.res.ns() {
                Some(Namespace::TypeNS) => namespaces[0] = true,
                Some(Namespace::ValueNS) => namespaces[1] = true,
                Some(Namespace::MacroNS) => namespaces[2] = true,
                None => {}
            }
        }
        if !found {
            return None;
        }
        let distinct = namespaces.iter().filter(|present| **present).count();
        Some(if distinct > 1 {
            Resolution::Ambiguous
        } else {
            Resolution::Unique
        })
    }
}

/// Where a resolved candidate's target lives relative to the documented
/// item's module — the axis [`ReferenceScope`] filters on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Defined directly in the documenting item's own module (not an
    /// import at all).
    LocalDefinition,
    /// Defined within that module's own subtree, reached through a
    /// `use self::...` of a descendant module's item.
    InternalImport,
    /// Reached from outside the module: a `use super::...` /
    /// `use crate::...` of another module, or an import from another
    /// crate.
    Outside,
}

/// Classify where `target` lives relative to the `scope` module, by the
/// defining module's position in the module tree (not the `use` path's
/// spelling). An external (non-local) target is always [`Reach::Outside`].
fn import_reach(cx: &LateContext<'_>, scope: LocalDefId, target: DefId) -> Reach {
    if target.as_local().is_none() {
        return Reach::Outside;
    }
    let scope_def = scope.to_def_id();
    let Some(defining_module) = cx.tcx.opt_parent(target) else {
        return Reach::Outside;
    };
    if defining_module == scope_def {
        return Reach::LocalDefinition;
    }
    // Walk the defining module's ancestry; reaching the scope module
    // means the target sits in the scope's own subtree.
    let mut module = Some(defining_module);
    while let Some(current) = module {
        if current == scope_def {
            return Reach::InternalImport;
        }
        module = cx.tcx.opt_parent(current);
    }
    Reach::Outside
}

/// The [`LocalDefId`] of the item a doc comment documents, for the
/// public-vs-private visibility check. Fields and enum variants carry
/// their own `def_id`; every other anchor (items, trait / impl /
/// foreign items, and the crate root) is an HIR owner, so its
/// `hir_id.owner` is the right def id.
fn documented_def_id(cx: &LateContext<'_>, hir_id: hir::HirId) -> LocalDefId {
    match cx.tcx.hir_node(hir_id) {
        hir::Node::Field(field) => field.def_id,
        hir::Node::Variant(variant) => variant.def_id,
        _ => hir_id.owner.def_id,
    }
}

/// The module whose scope a rustdoc intra-doc link on the node at
/// `hir_id` resolves in. For a module (or the crate root) that is the
/// node itself — its own `//!` / `///` doc resolves against its
/// contents; for any other item it is the enclosing module.
fn scope_module(cx: &LateContext<'_>, hir_id: hir::HirId) -> LocalDefId {
    match cx.tcx.hir_node(hir_id) {
        hir::Node::Crate(_) => CRATE_DEF_ID,
        hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Mod(..)) => {
            item.owner_id.def_id
        }
        _ => cx.tcx.parent_module(hir_id).to_local_def_id(),
    }
}

fn emit(
    cx: &LateContext<'_>,
    hir_id: hir::HirId,
    span: Span,
    violation: &Violation,
    resolution: Resolution,
) {
    let Violation { ident, snippet } = violation;
    span_lint_hir_and_then(
        cx,
        INTRA_DOC_LINKS,
        hir_id,
        span,
        format!("`{ident}` resolves in scope; write it as an intra-doc link"),
        |diag| match resolution {
            Resolution::Unique => {
                diag.span_suggestion(
                    span,
                    "wrap as an intra-doc link",
                    format!("[{snippet}]"),
                    Applicability::MachineApplicable,
                );
            }
            Resolution::Ambiguous => {
                diag.help(format!(
                    "`{ident}` resolves in more than one namespace; write a \
                     disambiguated intra-doc link such as `[`{ident}`](type@{ident})`",
                ));
            }
        },
    );
}
