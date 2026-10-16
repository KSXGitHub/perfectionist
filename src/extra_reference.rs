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
//! not `Copy`, there being no value for the autoderef to produce. That
//! holds only where the probe picks the *same* method, and the extra
//! reference adds a step at the front where another candidate can match
//! first: `[T; N]::into_iter` yields `T` where `&[T; N]`'s yields `&T`,
//! and `<&T as Clone>::clone` hands back the reference where `T`'s hands
//! back a `T`. An inherent method cannot be the one intercepted, there
//! being no way to write an inherent impl on a reference type, so a trait
//! method is declined rather than guessed at.
//!
//! What that leaves is an inherent method shadowed at the extra reference
//! by a trait method of the same name implemented for `&Item`. It needs
//! the trait in scope and the names to collide, and no cheap question
//! tells it apart from the inherent call, so it stays.
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
use rustc_middle::ty::adjustment::{Adjust, PointerCoercion};
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
            is_inherent(cx, parent) && (borrows_the_receiver(cx, parent) || copy)
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

/// Whether `call`'s method is an inherent one, which no impl on a
/// reference can intercept.
fn is_inherent<'tcx>(cx: &LateContext<'tcx>, call: &Expr<'tcx>) -> bool {
    cx.typeck_results()
        .type_dependent_def_id(call.hir_id)
        .is_some_and(|method| cx.tcx.trait_of_assoc(method).is_none())
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
    // Deref coercion walks the whole chain, so `&&String` reaches `&str`.
    // An unsize coercion is the one with no step to repeat: the folded
    // `&[u8; 32]` to `&[u8]` has no `&&[u8; 32]` form, and neither does
    // `&Concrete` to `&dyn Marker`.
    if cx
        .typeck_results()
        .expr_adjustments(mention)
        .iter()
        .any(|adjustment| matches!(adjustment.kind, Adjust::Pointer(PointerCoercion::Unsize)))
    {
        return false;
    }
    let declared = cx.tcx.fn_sig(called).skip_binder().skip_binder();
    let Some(parameter) = declared.inputs().get(at + offset) else {
        return false;
    };
    parameter.is_ref() && !parameter.has_param()
}
