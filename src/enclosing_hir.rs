//! Shared helper for rules that discover violation spans outside the
//! HIR walk and need to emit them at the enclosing HIR node so
//! `cfg_attr`-wrapped `#[expect]` / `#[allow]` attributes resolve
//! correctly.
//!
//! Two families of rules use it:
//!
//! - The pre-expansion → late-pass split of `macro_trailing_comma` and
//!   `macro_argument_binding`. They park macro-call spans during a
//!   pre-expansion pass and, in a late pass, anchor each at the deepest
//!   enclosing HIR node via [`find_enclosing_hir_ids`].
//! - The comment-walking rules (`bare_url`, `bare_email`,
//!   `bare_issue_reference`, `unicode_ellipsis_in_comments`,
//!   `unicode_ellipsis_in_docs`). They scan source text in a late pass
//!   and emit through [`emit_at_enclosing_hir`], which uses the
//!   attribute-aware [`find_comment_anchor_hir_ids`] so a doc comment
//!   resolves to the item it documents.
//!
//! Callers feed in the spans they care about and get back, for each
//! one, the deepest HIR node whose span contains it (or
//! [`hir::CRATE_HIR_ID`] if nothing did). Pre-expansion-pass payloads
//! that carry more than a span (e.g. `macro_trailing_comma`'s
//! `Insert` / `Remove` discriminator) project to `Span` at the call
//! site before invoking [`find_enclosing_hir_ids`].

use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

/// Walk the HIR once and, for each input span, return the deepest HIR
/// node whose own span contains it. The returned vector has the same
/// length and order as `target_spans`. A span not contained by any
/// visited node — e.g. one inside a synthesised macro expansion whose
/// enclosing item's span does not cover the call site — maps to
/// [`hir::CRATE_HIR_ID`].
pub(crate) fn find_enclosing_hir_ids(tcx: TyCtxt<'_>, target_spans: &[Span]) -> Vec<hir::HirId> {
    walk(tcx, target_spans, false)
}

/// Like [`find_enclosing_hir_ids`], but a node's outer attributes —
/// including the `#[doc]` attributes that `///` / `//!` / `/** */` doc
/// comments lower to — count toward containment alongside the node's
/// own span.
///
/// This is what the comment-walking rules need. An item's own span
/// starts at the item keyword, *after* its leading doc comment, so a
/// `…` inside `/// …` is not contained by the documented item's span
/// and plain [`find_enclosing_hir_ids`] would resolve it to the
/// enclosing module / crate. Folding each node's attribute spans in
/// re-attaches the doc comment to the item it documents, so a per-item
/// / per-field / per-variant `#[allow]` / `#[expect]` resolves. A plain
/// `//` / `/* */` comment carries no attribute, so it still anchors at
/// the deepest node whose body span contains it (the enclosing block /
/// item), which is the same place a user puts the suppressing
/// attribute.
fn find_comment_anchor_hir_ids(tcx: TyCtxt<'_>, target_spans: &[Span]) -> Vec<hir::HirId> {
    walk(tcx, target_spans, true)
}

fn walk(tcx: TyCtxt<'_>, target_spans: &[Span], include_attr_spans: bool) -> Vec<hir::HirId> {
    let mut best: Vec<hir::HirId> = vec![hir::CRATE_HIR_ID; target_spans.len()];
    let mut best_width: Vec<u32> = vec![u32::MAX; target_spans.len()];
    let mut finder = EnclosingHirFinder {
        tcx,
        targets: target_spans,
        best: &mut best,
        best_width: &mut best_width,
        include_attr_spans,
    };
    tcx.hir_walk_toplevel_module(&mut finder);
    best
}

/// Resolve each violation's primary span to its deepest enclosing HIR
/// node — in a single [`find_comment_anchor_hir_ids`] walk — then hand
/// that node id, the span, and the payload to `emit`.
///
/// The companion to [`find_enclosing_hir_ids`] for the comment-walking
/// rules (`bare_url`, `bare_email`, `bare_issue_reference`, the two
/// `unicode_ellipsis_in_*` rules). Those discover violation spans by
/// scanning source text in a late pass, outside the HIR walk, so the
/// early-pass lint-level builder would sit at the crate root at
/// emission time and only a crate-root `#![allow]` / `#![expect]`
/// would apply. Anchoring each diagnostic at its enclosing node — and
/// emitting through `clippy_utils::diagnostics::span_lint_hir_and_then`
/// from `emit` — is what lets a per-item / per-field / per-module
/// `#[allow]` / `#[expect]` resolve.
pub(crate) fn emit_at_enclosing_hir<Payload>(
    tcx: TyCtxt<'_>,
    violations: Vec<(Span, Payload)>,
    mut emit: impl FnMut(hir::HirId, Span, Payload),
) {
    if violations.is_empty() {
        return;
    }
    let target_spans: Vec<Span> = violations.iter().map(|(span, _)| *span).collect();
    let hir_ids = find_comment_anchor_hir_ids(tcx, &target_spans);
    for ((span, payload), hir_id) in violations.into_iter().zip(hir_ids) {
        emit(hir_id, span, payload);
    }
}

struct EnclosingHirFinder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    targets: &'a [Span],
    best: &'a mut [hir::HirId],
    /// Byte width of the span that elected each `best[i]`, used only in
    /// comment-anchoring mode to keep the *tightest* containing node
    /// (see [`Self::update`]). `u32::MAX` until the first match.
    best_width: &'a mut [u32],
    /// When set, a documentable node's `#[doc]` attribute spans (what
    /// `///` / `//!` / `/** */` lower to) count toward containment
    /// alongside its own span, and enum variants / struct fields are
    /// registered as anchors. See [`find_comment_anchor_hir_ids`].
    include_attr_spans: bool,
}

impl<'a, 'tcx> EnclosingHirFinder<'a, 'tcx> {
    /// Record a node that can carry a doc comment: its own `span` and,
    /// in comment-anchoring mode, the span of each `#[doc]` attribute it
    /// bears. Folding the attribute spans in is what attaches a `///` /
    /// `//!` / `/** */` doc comment to the item it documents, whose own
    /// span begins after the comment.
    ///
    /// Only the node kinds a doc comment can actually attach to —
    /// items, trait / impl / foreign items, enum variants, struct
    /// fields — route through here. Blocks, statements, locals,
    /// expressions, and patterns never carry a doc comment, so they
    /// call [`Self::update`] directly and skip the per-node
    /// `hir_attrs` lookup.
    fn register_documentable(&mut self, hir_id: hir::HirId, span: Span) {
        self.update(hir_id, span);
        if self.include_attr_spans {
            // `is_doc_comment` hands back the comment's own span; other
            // attributes are skipped, which also sidesteps
            // `Attribute::span` panicking on parsed attribute kinds that
            // carry no span (e.g. the synthesised prelude import).
            for attr in self.tcx.hir_attrs(hir_id) {
                if let Some(doc_span) = attr.is_doc_comment() {
                    self.update(hir_id, doc_span);
                }
            }
        }
    }

    fn update(&mut self, hir_id: hir::HirId, span: Span) {
        for (index, &target) in self.targets.iter().enumerate() {
            if self.include_attr_spans {
                // Comment-anchoring mode. Resolve macro hygiene before
                // comparing: a `///` forwarded through a `macro_rules!`
                // (`declare_tool_lint!` does this for every lint here)
                // lands on the generated item with an
                // expansion-context `#[doc]` span that does not
                // byte-match the root-context comment the walker
                // scanned, so a raw `Span::contains` would miss it and
                // the finding would fall back to the crate root.
                let Some(width) = comment_enclosure_width(span, target) else {
                    continue;
                };
                // Keep the *tightest* enclosing node, measured at the
                // source level (see [`comment_enclosure_width`]), not
                // the last-visited one. A proc-macro `#[derive]`
                // (e.g. `serde::Deserialize`) generates root-context
                // HIR nodes — `visit_map` / `visit_seq` bodies — whose
                // spans cover the whole field list and are visited
                // *after* the struct's fields. A depth/order tie-break
                // would let those wider generated nodes steal the
                // anchor from the documented field, defeating a
                // field-level `#[expect]` (issue #165 follow-up).
                // Preferring the narrowest span keeps the field's own
                // one-line `#[doc]` span.
                if width >= self.best_width[index] {
                    continue;
                }
                self.best_width[index] = width;
                self.best[index] = hir_id;
            } else {
                // Macro-call mode. Targets here are pre-expansion call
                // sites, so [`contains`] resolves macro hygiene before
                // comparing byte ranges. The walk is depth-first, so a
                // parent is visited before its children and the last
                // successful update lands on the deepest node seen.
                if !contains(span, target) {
                    continue;
                }
                self.best[index] = hir_id;
            }
        }
    }
}

/// Containment check that resolves macro hygiene before comparing byte
/// ranges. A HIR item synthesised by a macro expansion can carry an
/// `Item.span` whose byte positions point into the macro definition
/// body (def-site), not into the call site — for example, the
/// `pub const $name: $ty = $value;` template inside a `macro_rules!`
/// block. A direct byte-range check against a pre-expansion target
/// span (which sits at the call site) misses such an item, so the
/// `best[index]` slot lands on a *child* HIR node of the expanded
/// item (one of the captures, which does carry a call-site span)
/// rather than on the item itself.
///
/// `#[expect]` / `#[allow]` resolution walks HIR ancestry from the
/// anchor up, so this child-node anchoring still surfaces attributes
/// on the surrounding module today; the fallback is a semantic
/// improvement (the diagnostic now anchors at the expanded item, not
/// at one of its captures) rather than a fix for an observable bug.
/// It also guards against future shapes where no descendant carries
/// a call-site span — items the visitor doesn't recurse into, or
/// proc-macro expansions that set spans atypically.
///
/// Resolving both spans through [`Span::source_callsite`] walks each
/// span up its expansion chain until it lands on user-written source.
/// For an expanded item the call-site span byte-covers the call's
/// arguments, so the containment check succeeds and the deepest HIR
/// node wins as intended.
fn contains(item_span: Span, target: Span) -> bool {
    item_span.contains(target)
        || item_span
            .source_callsite()
            .contains(target.source_callsite())
}

/// For comment anchoring: if `candidate` encloses `target`, return the
/// byte width to tie-break on (smaller = tighter); otherwise `None`.
///
/// Containment is checked the same hygiene-resolving way as [`contains`]
/// — a direct byte check first, then a [`Span::source_callsite`] check
/// so a `#[doc]` forwarded through a `macro_rules!` (whose span sits in
/// the macro expansion) still matches the generated item it landed on.
///
/// The width is measured at the level the match held: the candidate's
/// own span for a direct match, or its `source_callsite` for a
/// hygiene match. Measuring the hygiene case at the resolved source
/// span is what lets a macro-generated item's `#[doc]` (resolving to
/// the one-line `///`) win the tightest-node tie-break over the
/// enclosing module, while a root-context proc-macro-`derive` body
/// keeps its wide field-list width and loses to the documented field.
fn comment_enclosure_width(candidate: Span, target: Span) -> Option<u32> {
    if candidate.contains(target) {
        return Some(span_width(candidate));
    }
    let resolved = candidate.source_callsite();
    if resolved.contains(target.source_callsite()) {
        return Some(span_width(resolved));
    }
    None
}

fn span_width(span: Span) -> u32 {
    span.hi().0.saturating_sub(span.lo().0)
}

impl<'tcx> Visitor<'tcx> for EnclosingHirFinder<'_, 'tcx> {
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        self.register_documentable(item.hir_id(), item.span);
        intravisit::walk_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'tcx hir::TraitItem<'tcx>) {
        self.register_documentable(item.hir_id(), item.span);
        intravisit::walk_trait_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'tcx hir::ImplItem<'tcx>) {
        self.register_documentable(item.hir_id(), item.span);
        intravisit::walk_impl_item(self, item);
    }

    fn visit_foreign_item(&mut self, item: &'tcx hir::ForeignItem<'tcx>) {
        self.register_documentable(item.hir_id(), item.span);
        intravisit::walk_foreign_item(self, item);
    }

    // Variants and fields are only registered in comment-anchoring mode
    // (where a doc comment can anchor to them). The macro-call path
    // (`find_enclosing_hir_ids`) never visited them, so guarding on the
    // flag keeps that path byte-for-byte unchanged; the walk still
    // recurses into them either way to reach nested expressions.
    fn visit_variant(&mut self, variant: &'tcx hir::Variant<'tcx>) {
        if self.include_attr_spans {
            self.register_documentable(variant.hir_id, variant.span);
        }
        intravisit::walk_variant(self, variant);
    }

    fn visit_field_def(&mut self, field: &'tcx hir::FieldDef<'tcx>) {
        if self.include_attr_spans {
            self.register_documentable(field.hir_id, field.span);
        }
        intravisit::walk_field_def(self, field);
    }

    fn visit_block(&mut self, block: &'tcx hir::Block<'tcx>) {
        self.update(block.hir_id, block.span);
        intravisit::walk_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &'tcx hir::Stmt<'tcx>) {
        self.update(stmt.hir_id, stmt.span);
        intravisit::walk_stmt(self, stmt);
    }

    fn visit_local(&mut self, local: &'tcx hir::LetStmt<'tcx>) {
        self.update(local.hir_id, local.span);
        intravisit::walk_local(self, local);
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        self.update(expr.hir_id, expr.span);
        intravisit::walk_expr(self, expr);
    }

    fn visit_pat(&mut self, pat: &'tcx hir::Pat<'tcx>) {
        self.update(pat.hir_id, pat.span);
        intravisit::walk_pat(self, pat);
    }
}
