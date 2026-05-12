//! Shared helper for the pre-expansion → late-pass split that
//! `macro_trailing_comma` and `macro_argument_binding` both use.
//!
//! Both rules emit their diagnostics from a late pass, after parking
//! violation spans during a pre-expansion pass. The late pass then
//! anchors each pending span at the deepest enclosing HIR node so
//! `cfg_attr`-wrapped `#[expect]` / `#[allow]` attributes resolve
//! correctly. The walk shape is identical between the two rules; this
//! module provides the generic walker.
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
/// visited node — e.g. one that lies outside the crate's local HIR —
/// maps to [`hir::CRATE_HIR_ID`].
pub(crate) fn find_enclosing_hir_ids(tcx: TyCtxt<'_>, target_spans: &[Span]) -> Vec<hir::HirId> {
    let mut best: Vec<hir::HirId> = vec![hir::CRATE_HIR_ID; target_spans.len()];
    let mut finder = EnclosingHirFinder {
        tcx,
        targets: target_spans,
        best: &mut best,
    };
    tcx.hir_walk_toplevel_module(&mut finder);
    best
}

struct EnclosingHirFinder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    targets: &'a [Span],
    best: &'a mut [hir::HirId],
}

impl<'a, 'tcx> EnclosingHirFinder<'a, 'tcx> {
    fn update(&mut self, hir_id: hir::HirId, span: Span) {
        for (index, &target) in self.targets.iter().enumerate() {
            if !span.contains(target) {
                continue;
            }
            // The walk is depth-first: a parent is visited before its
            // children, so each successful containment update lands on
            // the deepest node seen so far.
            self.best[index] = hir_id;
        }
    }
}

impl<'tcx> Visitor<'tcx> for EnclosingHirFinder<'_, 'tcx> {
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'tcx hir::TraitItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_trait_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'tcx hir::ImplItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_impl_item(self, item);
    }

    fn visit_foreign_item(&mut self, item: &'tcx hir::ForeignItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_foreign_item(self, item);
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
