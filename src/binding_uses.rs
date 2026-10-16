//! Every use of a local binding inside an expression, nested closure
//! bodies included.
//!
//! The nesting filter is the point. A visitor left at its default never
//! enters a nested body, so a closure argument naming the binding hides
//! from it. Each rule asking this question asks it about an expression
//! that is going to move into an adapter of its own, where a name the
//! walk missed would be left behind: a step's closure naming the item,
//! or a combinator's closure naming it.

use rustc_hir::def::Res;
use rustc_hir::intravisit::{Visitor, walk_expr};
use rustc_hir::{Expr, ExprKind, HirId, QPath};
use rustc_lint::LateContext;
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;

/// Every use of any of `bindings` inside `expr`, in source order.
pub(crate) fn uses<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    bindings: &[HirId],
) -> Vec<&'tcx Expr<'tcx>> {
    struct Collect<'a, 'tcx> {
        tcx: TyCtxt<'tcx>,
        bindings: &'a [HirId],
        found: Vec<&'tcx Expr<'tcx>>,
    }
    impl<'tcx> Visitor<'tcx> for Collect<'_, 'tcx> {
        type NestedFilter = nested_filter::OnlyBodies;

        fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
            self.tcx
        }

        fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
            if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind
                && let Res::Local(local) = path.res
                && self.bindings.contains(&local)
            {
                self.found.push(expr);
            }
            walk_expr(self, expr);
        }
    }
    let mut collect = Collect {
        tcx: cx.tcx,
        bindings,
        found: Vec::new(),
    };
    collect.visit_expr(expr);
    collect.found
}

/// Whether `expr` names any of `bindings`.
pub(crate) fn names<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    bindings: &[HirId],
) -> bool {
    !uses(cx, expr, bindings).is_empty()
}
