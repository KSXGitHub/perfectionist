use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::def::{DefKind, Namespace, Res};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::{CRATE_DEF_ID, LocalDefId};
use rustc_span::{Span, Symbol};

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolve_symbol_set, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;

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

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Identifiers the rule never suggests linking, even when they
    /// resolve in scope. Empty by default. Use this for a name a doc
    /// comment deliberately mentions without wanting a cross-reference
    /// — a historical type kept for context, or a word that happens to
    /// collide with an in-scope item but is meant as prose.
    skip_idents: Vec<String>,
}

pub struct IntraDocLinks {
    skip_idents: BTreeSet<Symbol>,
}

impl IntraDocLinks {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let skip_idents = resolve_symbol_set(&[], config.skip_idents, Vec::new());
        Self { skip_idents }
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
            if let Some(resolution) = resolve_in_scope(lint_context, hir_id, violation.ident) {
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

/// Resolve `name` against the children of the documented item's scope
/// module. Returns `None` when the name names nothing in scope (so the
/// backticks are deliberate prose, not an unlinked reference).
fn resolve_in_scope(cx: &LateContext<'_>, hir_id: hir::HirId, name: Symbol) -> Option<Resolution> {
    let scope = scope_module(cx, hir_id);
    // One slot per namespace (`TypeNS`, `ValueNS`, `MacroNS`); a name
    // present in more than one is an ambiguous intra-doc link.
    let mut namespaces = [false; 3];
    let mut found = false;
    for child in cx.tcx.module_children_local(scope) {
        if child.ident.name != name {
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
