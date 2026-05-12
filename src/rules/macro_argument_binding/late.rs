//! Late-pass machinery: drains the pre-expansion pass's
//! [`PENDING_VIOLATIONS`] queue and emits each diagnostic at the
//! deepest enclosing HIR node so `cfg_attr`-wrapped `#[expect]` /
//! `#[allow]` attributes resolve correctly.
//!
//! Mirrors the equivalent late pass in `macro_trailing_comma`; the
//! two cannot share a single `EnclosingHirFinder` instance cheaply
//! because each rule's pending list is a different type, but the walk
//! shape is identical and a future refactor can extract a generic
//! helper.

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use super::{MACRO_ARGUMENT_BINDING, PENDING_VIOLATIONS};

pub(super) struct MacroArgumentBindingLate;

impl<'tcx> LateLintPass<'tcx> for MacroArgumentBindingLate {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<Span> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            std::mem::take(&mut *guard)
        };
        if pending.is_empty() {
            return;
        }
        let tcx = lint_context.tcx;
        let mut best: Vec<hir::HirId> = vec![hir::CRATE_HIR_ID; pending.len()];
        let mut finder = EnclosingHirFinder {
            tcx,
            pending: &pending,
            best: &mut best,
        };
        tcx.hir_walk_toplevel_module(&mut finder);
        for (&span, &hir_id) in pending.iter().zip(best.iter()) {
            emit(lint_context, hir_id, span);
        }
    }
}

fn emit(lint_context: &LateContext<'_>, hir_id: hir::HirId, span: Span) {
    span_lint_hir_and_then(
        lint_context,
        MACRO_ARGUMENT_BINDING,
        hir_id,
        span,
        "non-trivial expression passed directly to a macro",
        |diag| {
            diag.help(
                "bind the expression to a `let` immediately before the macro \
                 call so it is evaluated exactly once, regardless of how the \
                 macro expands",
            );
        },
    );
}

/// Walk the HIR once and, for each pending violation span, record the
/// deepest HIR node whose span contains it. Mirrors the equivalent
/// finder in `macro_trailing_comma`.
struct EnclosingHirFinder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pending: &'a [Span],
    best: &'a mut [hir::HirId],
}

impl<'a, 'tcx> EnclosingHirFinder<'a, 'tcx> {
    fn update(&mut self, hir_id: hir::HirId, span: Span) {
        for (index, &target) in self.pending.iter().enumerate() {
            if !span.contains(target) {
                continue;
            }
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
