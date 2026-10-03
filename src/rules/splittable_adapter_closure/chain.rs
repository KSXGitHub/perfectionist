//! Finding the chain of steps rooted at the closure's item parameter,
//! and collecting what else names it.
//!
//! The chain is found by walking *up* from the item's occurrence rather
//! than down from the body, because what makes a parent a step is that
//! the chain so far is the thing it is applied to. Walking down would
//! have to guess which sub-expression the chain runs through.

use crate::binding_uses::names;
use rustc_hir::{Expr, ExprKind, HirId, Node, Pat, PatKind};
use rustc_lint::LateContext;

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
                    && !arguments
                        .iter()
                        .any(|argument| names(cx, argument, parameters))
            }
            ExprKind::Call(callee, [only]) => {
                only.hir_id == chain.hir_id && !names(cx, callee, parameters)
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
