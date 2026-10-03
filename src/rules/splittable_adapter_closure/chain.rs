//! Finding the chain of steps rooted at the closure's item parameter,
//! and collecting what else names it.
//!
//! The chain is found by walking *up* from the item's occurrence rather
//! than down from the body, because what makes a parent a step is that
//! the chain so far is the thing it is applied to. Walking down would
//! have to guess which sub-expression the chain runs through.

use rustc_hir::def::Res;
use rustc_hir::intravisit::{Visitor, walk_expr};
use rustc_hir::{Body, Expr, ExprKind, HirId, Node, Pat, PatKind, QPath};
use rustc_lint::LateContext;
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;

/// One step of the chain, and the expression it is applied to.
pub(super) struct Step<'tcx> {
    /// The whole step, so `a.trim()` rather than `trim`.
    pub(super) expr: &'tcx Expr<'tcx>,
}

/// The binding an item parameter introduces, or `None` for a pattern
/// that is not one plain binding.
///
/// A destructured parameter bottoms the chain out at a binding the
/// pattern introduced rather than at the parameter, which the walk
/// cannot start from. Reproducing the pattern in the lifted `map` is a
/// different rewrite from lifting a step.
pub(super) fn binding(pat: &Pat<'_>) -> Option<HirId> {
    match pat.kind {
        PatKind::Binding(_, hir_id, _, None) => Some(hir_id),
        _ => None,
    }
}

/// Every use of `item` in `body`, in source order.
///
/// Collected rather than left to the walk, which constrains only the
/// positions it looks at: a call's callee, or a step's own closure
/// argument, can name the item where the walk never goes. Each step gets
/// its own closure after the split, so a second use would be left with
/// nothing to name.
pub(super) fn occurrences<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    item: HirId,
) -> Vec<&'tcx Expr<'tcx>> {
    uses(cx, body.value, &[item])
}

/// The steps rooted at `root`, outermost last.
///
/// A parent is a step where it applies something to the chain so far: a
/// method call whose receiver is the chain, or a call whose sole
/// argument is it. The walk stops at the first parent that is neither,
/// which is what leaves a step naming the accumulator outside the chain.
///
/// "Sole argument" is deliberately narrow. `foo(bar(item), 1)` does
/// split, but recognising it means picking which argument carries the
/// chain, and picking wrong suggests code that does not compile.
pub(super) fn steps<'tcx>(
    cx: &LateContext<'tcx>,
    root: &'tcx Expr<'tcx>,
    parameters: &[HirId],
) -> Vec<Step<'tcx>> {
    let mut steps = Vec::new();
    let mut chain = root;
    while let Node::Expr(parent) = cx.tcx.parent_hir_node(chain.hir_id) {
        // `?` lowers to a `match` on `Try::branch(operand)`, and that
        // call's sole argument is the chain, so it reads as a step the
        // reader never wrote and could not lift into a `map`.
        if parent.span.desugaring_kind().is_some() {
            break;
        }
        let extends = match parent.kind {
            ExprKind::MethodCall(_, receiver, arguments, _) => {
                receiver.hir_id == chain.hir_id
                    && arguments
                        .iter()
                        .all(|argument| uses(cx, argument, parameters).is_empty())
            }
            ExprKind::Call(callee, [only]) => {
                only.hir_id == chain.hir_id && uses(cx, callee, parameters).is_empty()
            }
            _ => false,
        };
        if !extends {
            break;
        }
        steps.push(Step { expr: parent });
        chain = parent;
    }
    steps
}

/// Every use of any of `bindings` inside `expr`, nested closure bodies
/// included.
///
/// A nested body is where a use hides from a visitor left at the default
/// nesting filter: a step's own closure argument can name the item or
/// the accumulator, and lifting that step would leave the name behind.
pub(super) fn uses<'tcx>(
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
pub(super) fn mentions<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    bindings: &[HirId],
) -> bool {
    !uses(cx, expr, bindings).is_empty()
}
