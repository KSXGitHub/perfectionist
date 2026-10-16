//! Whether a lifted adapter may take the receiver the folded one has.
//!
//! A split lifts work into an adapter placed in front of the one it came
//! from, and the three adapters the work lifts into are `map`, `filter`
//! and `take_while`, which all take the iterator by value. The adapter the
//! work came from need not: `Iterator::any`, `find`, `rfind`, `position`
//! and `find_map` take `&mut self`, so the folded form leaves the
//! receiver where it was and the split moves it away.
//!
//! That is only a problem where the receiver is something a move cannot
//! be taken from, so the question is asked of the method first and of
//! the receiver second.

use crate::binding_uses::uses;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, HirId, Node, QPath, UnOp};
use rustc_lint::LateContext;

/// Whether the lifted adapter can be given `receiver`, where `call` is
/// the folded adapter's own call.
pub(crate) fn movable<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Expr<'tcx>,
    receiver: &'tcx Expr<'tcx>,
) -> bool {
    if !borrows_the_receiver(cx, call) {
        return true;
    }
    match receiver.kind {
        // A local is movable where this call is the only thing naming it,
        // and the call runs at most once per time the local is bound:
        // anything else naming it is `E0382` once the split moves it, and
        // whether that other mention runs first is a question a span
        // cannot answer.
        ExprKind::Path(QPath::Resolved(None, path)) => match path.res {
            Res::Local(local) => moved_once(cx, call, local),
            _ => false,
        },
        // A place the receiver does not own: moving out of one is
        // `E0507`, and a `Copy` iterator is rare enough to decline with
        // the rest.
        ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Unary(UnOp::Deref, _) => false,
        // Everything else is a value the folded form made, which nothing
        // else can be holding.
        _ => true,
    }
}

/// Whether the method `call` resolves to takes its receiver by
/// reference, which is what leaves the receiver usable after it.
pub(crate) fn borrows_the_receiver<'tcx>(cx: &LateContext<'tcx>, call: &Expr<'tcx>) -> bool {
    let Some(method) = cx.typeck_results().type_dependent_def_id(call.hir_id) else {
        return false;
    };
    cx.tcx
        .fn_sig(method)
        .skip_binder()
        .skip_binder()
        .inputs()
        .first()
        .is_some_and(|receiver| receiver.is_ref())
}

/// Whether the split can move `local` out from under `call`.
///
/// The body to ask is the one that declares `local`, not the innermost
/// one: a chain inside a nested closure names an upvar, and what the
/// enclosing function does with that local afterwards is exactly what the
/// move would break.
///
/// Naming it once is not enough either, because a mention inside a loop,
/// or inside a closure the body may call again, is evaluated more than
/// once however often it is written, and the second evaluation moves an
/// already-moved local.
fn moved_once<'tcx>(cx: &LateContext<'tcx>, call: &Expr<'tcx>, local: HirId) -> bool {
    let owner = cx.tcx.hir_enclosing_body_owner(local);
    let body = cx.tcx.hir_body_owned_by(owner);
    uses(cx, body.value, &[local]).len() == 1 && evaluated_once(cx, call.hir_id, body.value.hir_id)
}

/// Whether nothing between `call` and the body root can run it twice.
fn evaluated_once(cx: &LateContext<'_>, call: HirId, body: HirId) -> bool {
    let mut child = call;
    while child != body {
        match cx.tcx.parent_hir_node(child) {
            Node::Expr(parent) => {
                if matches!(parent.kind, ExprKind::Loop(..) | ExprKind::Closure(..)) {
                    return false;
                }
                child = parent.hir_id;
            }
            Node::Stmt(statement) => child = statement.hir_id,
            Node::LetStmt(local) => child = local.hir_id,
            Node::Block(block) => child = block.hir_id,
            Node::ExprField(field) => child = field.hir_id,
            Node::Arm(arm) => child = arm.hir_id,
            _ => return false,
        }
    }
    true
}
