//! Whether work lifted out of an adapter survives being handed one
//! reference more than it had.
//!
//! Two rules move work from an adapter handed the item itself into
//! `filter` or `take_while`, which hand `&Item`. Every mention of the
//! item in the lifted part then sits one reference deeper, and only two
//! positions survive that.
//!
//! A method call's receiver autoderefs to whatever depth the method
//! wants, so it survives unless the method takes `self` and the item is
//! not `Copy`, there being no value for the autoderef to produce.
//!
//! An argument survives where the parameter it fills is declared as a
//! reference to a type holding no parameter, a coercion site derefing
//! `&&T` to `&T`. A parameter naming a type parameter is not one: the
//! extra reference is inferred into the parameter instead, and the bound
//! is then checked against it. `HashSet::<String>::contains` takes
//! `&Q where String: Borrow<Q>`, which `Q = &String` does not satisfy.

use crate::binding_uses::uses;
use crate::receiver_move::borrows_the_receiver;
use rustc_hir::{Expr, ExprKind, HirId, Node};
use rustc_lint::LateContext;
use rustc_middle::ty::{Ty, TyKind, TypeVisitableExt};

/// Whether every mention of `item` in `lifted` survives one more
/// reference.
pub(crate) fn survives<'tcx>(
    cx: &LateContext<'tcx>,
    lifted: &'tcx Expr<'tcx>,
    item: HirId,
    item_ty: Ty<'tcx>,
) -> bool {
    let copy = cx.type_is_copy_modulo_regions(item_ty);
    uses(cx, lifted, &[item])
        .iter()
        .all(|mention| mention_survives(cx, mention, copy))
}

fn mention_survives<'tcx>(cx: &LateContext<'tcx>, mention: &Expr<'tcx>, copy: bool) -> bool {
    let Node::Expr(parent) = cx.tcx.parent_hir_node(mention.hir_id) else {
        return false;
    };
    match parent.kind {
        ExprKind::MethodCall(_, receiver, _, _) if receiver.hir_id == mention.hir_id => {
            borrows_the_receiver(cx, parent) || copy
        }
        // `self` occupies the first declared parameter, so the arguments
        // start one along.
        ExprKind::MethodCall(_, _, arguments, _) => {
            let method = cx.typeck_results().type_dependent_def_id(parent.hir_id);
            fills_a_concrete_reference(cx, method, arguments, mention, 1)
        }
        ExprKind::Call(callee, arguments) if callee.hir_id != mention.hir_id => {
            let TyKind::FnDef(called, _) = cx.typeck_results().expr_ty(callee).kind() else {
                return false;
            };
            fills_a_concrete_reference(cx, Some(*called), arguments, mention, 0)
        }
        _ => false,
    }
}

/// Whether the parameter `mention` fills is declared as a reference to a
/// type holding no parameter.
fn fills_a_concrete_reference<'tcx>(
    cx: &LateContext<'tcx>,
    called: Option<rustc_hir::def_id::DefId>,
    arguments: &'tcx [Expr<'tcx>],
    mention: &Expr<'tcx>,
    offset: usize,
) -> bool {
    let Some(called) = called else {
        return false;
    };
    let Some(at) = arguments
        .iter()
        .position(|argument| argument.hir_id == mention.hir_id)
    else {
        return false;
    };
    let declared = cx.tcx.fn_sig(called).skip_binder().skip_binder();
    let Some(parameter) = declared.inputs().get(at + offset) else {
        return false;
    };
    parameter.is_ref() && !parameter.has_param()
}
