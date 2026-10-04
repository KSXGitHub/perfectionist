//! The chain trigger: a chain of two or more steps rooted at the
//! closure's item, so one adapter performs all of them.
//!
//! The trigger walks *up* from the item parameter's occurrence, because
//! what makes a parent a step is that the chain so far is what it is
//! applied to; walking down would have to guess which sub-expression the
//! chain runs through. [`super::chain`] does that walk, [`super::adapter`]
//! says which methods are in scope and where their item sits, and
//! [`super::anchoring`] answers the one question whose wrong answer
//! compiles.
//!
//! Three gates stand between the walk and a diagnostic, and they fail
//! differently. The item occurring more than once leaves a split with a
//! binding to name that no longer exists, so the suggestion would be
//! `E0425`. A step whose result borrows the item would be `E0515` once
//! lifted. A chain in a conditionally-evaluated position, though, splits
//! into something that compiles and behaves differently, which is why
//! [`super::anchoring`] is where the care goes.

use super::adapter::{self, Adapter};
use super::family::Family;
use super::{Call, Finding, anchoring, chain};
use crate::binding_uses::{names, uses};
use crate::common::borrows;
use crate::exclusive_captures::exclusive;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::Symbol;

/// The chain this adapter's closure holds, where it is two or more steps
/// long.
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Call<'tcx>,
    family: Family,
) -> Option<Finding> {
    let (method, arguments) = (call.method, call.arguments);
    let adapter = adapter::adapter(family, method)?;
    let argument = arguments.get(adapter.closure_argument)?;
    let ExprKind::Closure(closure) = argument.kind else {
        return None;
    };
    let body = cx.tcx.hir_body(closure.body);
    let parameter = body.params.get(adapter.item_parameter)?;
    let item = chain::binding(parameter.pat)?;
    // Each step gets its own closure after the split, so a second use of
    // the item would be left with nothing to name.
    let occurrences = uses(cx, body.value, &[item]);
    let [root] = *occurrences.as_slice() else {
        return None;
    };
    // A step moves into an adapter outside the closure, where nothing the
    // body declares is in scope, so a step naming any of it would be
    // `E0425` once lifted.
    let locals = chain::declared(body);
    let steps = chain::steps(cx, root, &locals);
    if steps.len() < 2 {
        return None;
    }
    // The last step stays with the adapter where the closure takes the
    // item alone, so only the ones moving out have to be liftable.
    let lifted = match adapter.keeps_the_last_step {
        true => &steps[..steps.len() - 1],
        false => &steps[..],
    };
    if !lifted.iter().all(|step| is_liftable(cx, step.expr)) {
        return None;
    }
    // A parallel adapter requires the item it produces to be `Send`,
    // where the folded form does not, because the value never leaves the
    // closure.
    if family.sends_between_threads() && !lifted.iter().all(|step| is_sendable(cx, step.expr)) {
        return None;
    }
    // A lifted step runs in a closure of its own, alongside the one the
    // adapter keeps. A capture only a shared borrow or a copy of a `Copy`
    // value lets both of them hold, so a step reaching any other one is
    // declined rather than worked out: whether the step is the only half
    // reaching it is a question the split's shape answers differently per
    // adapter.
    let held_alone = exclusive(cx, closure.def_id);
    if lifted.iter().any(|step| names(cx, step.expr, &held_alone)) {
        return None;
    }
    let top = steps.last().expect("two or more steps").expr;
    let anchored = match adapter.keeps_the_last_step {
        // Requiring the chain to *be* the body satisfies the position
        // condition vacuously, and holds the scope to a closure whose
        // whole job is the chain.
        true => top.hir_id == body.value.hir_id,
        false => {
            anchoring::always_evaluated(cx, top.hir_id, body.value.hir_id)
                && anchoring::nothing_observable_first(cx, top.hir_id, body.value.hir_id)
        }
    };
    if !anchored {
        return None;
    }
    Some(Finding {
        message: format!(
            "this closure chains {} steps onto the item, so `{method}` does all of them",
            steps.len(),
        ),
        help: help(adapter, method, lifted.len()),
        moves_the_receiver: !adapter.head_keeps_the_method,
        fix: None,
    })
}

/// What to tell the reader to do, which differs by whether the adapter
/// keeps a step and by whether the head of the split keeps the method.
fn help(adapter: Adapter, method: Symbol, lifted: usize) -> String {
    if adapter.head_keeps_the_method {
        return match lifted {
            1 => format!(
                "lift the first step into a leading `{method}` of its own, leaving \
                 the closure the step that is the adapter's",
            ),
            _ => format!(
                "lift the first step into a leading `{method}` and each later one \
                 into a `{}` of its own, leaving the closure the step that is the \
                 adapter's",
                adapter.lift_target,
            ),
        };
    }
    match adapter.keeps_the_last_step {
        true => format!(
            "lift all but the last step into a leading `{}` of its own, leaving \
             the closure the step that is the adapter's",
            adapter.lift_target,
        ),
        false => format!(
            "lift every step into a leading `{}` of its own, leaving the closure \
             the accumulation",
            adapter.lift_target,
        ),
    }
}

/// Whether lifting `step` out of the closure keeps it compiling.
///
/// A lifted step runs in a `map` of its own, applied to a local that
/// dies at the end of it, so a result borrowing that local is `E0515`.
/// Two shapes rule that out. A result carrying no lifetime borrows
/// nothing. And where what the step is applied to is itself a
/// reference, a `&self` borrows the referent rather than the local
/// pointer, and the referent outlives the closure:
/// `.map(|s: &str| s.trim())` hands back a borrow of what `s` points
/// at.
///
/// The question is about the step's own receiver, which is the item
/// only for the first step. Each later one is applied to the step
/// before it, so `.map(|s: &str| s.to_lowercase().trim())` has an
/// owned `String` under its `trim` however the item arrived.
///
/// A borrowing result is only the receiver's to answer for where nothing
/// else the step was handed could have lent it: `s.max(&String::from("m")[..])`
/// returns the shorter of two lifetimes, and the shorter one belongs to a
/// temporary the closure made. [`lends_nothing`] is that question.
///
/// Regions are erased by the time typeck results are read, so a
/// result's lifetime cannot be matched against the receiver's. These
/// clauses are the conservative answer that needs no such match.
fn is_liftable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    if !borrows(cx.typeck_results().expr_ty(step)) {
        return true;
    }
    let Some(receiver) = applied_to(step) else {
        return false;
    };
    if !cx.typeck_results().expr_ty(receiver).is_ref() {
        return false;
    }
    lends_nothing(cx, step)
}

/// Whether nothing `step` was handed besides its receiver could have lent
/// it the borrow its result carries.
///
/// A method call's arguments are read from the method's **declared**
/// signature rather than from the types the call instantiated. The
/// instantiated ones have their regions erased, where the signature still
/// names the regions and the type parameters each input and the output are
/// made of, so the question is whether the output shares any of them with
/// an argument. `str::strip_prefix<P>(&self, prefix: P) -> Option<&str>`
/// shares none: its result is made of the receiver's region, and the
/// pattern is a parameter of its own. `Ord::max(self, other: Self) -> Self`
/// shares `Self` with its argument, which is how it returns the shorter of
/// two lifetimes.
///
/// A call's sole argument is the chain, which is the receiver here, so
/// what is left to ask about is the callee: an expression yielding an
/// `Fn` that returns a borrow lends that borrow to the result. A path to
/// a function is not one, its regions belonging to the signature rather
/// than to anything the zero-sized item holds.
fn lends_nothing<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    match step.kind {
        ExprKind::MethodCall(..) => arguments_lend_nothing(cx, step),
        ExprKind::Call(callee, _) if !matches!(callee.kind, ExprKind::Path(_)) => {
            !borrows(cx.typeck_results().expr_ty(callee))
        }
        _ => true,
    }
}

/// Whether the declared signature of the method `step` calls makes its
/// output of nothing an argument carries.
fn arguments_lend_nothing<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    let Some(method) = cx.typeck_results().type_dependent_def_id(step.hir_id) else {
        return false;
    };
    let signature = cx.tcx.fn_sig(method).skip_binder().skip_binder();
    // The receiver is the step's own, which `is_liftable` has answered for.
    let [_receiver, arguments @ ..] = signature.inputs() else {
        return false;
    };
    let output = made_of(signature.output());
    !arguments
        .iter()
        .flat_map(|argument| made_of(*argument))
        .any(|part| output.contains(&part))
}

/// The regions and type parameters `ty` is made of, which is what two
/// declared types share where a value of the one can be made out of a
/// value of the other.
fn made_of<'tcx>(ty: Ty<'tcx>) -> Vec<ty::GenericArg<'tcx>> {
    ty.walk()
        .filter(|part| match part.kind() {
            ty::GenericArgKind::Lifetime(_) => true,
            ty::GenericArgKind::Type(inner) => matches!(inner.kind(), ty::Param(_)),
            ty::GenericArgKind::Const(_) => false,
        })
        .collect()
}

/// What `step` applies itself to: a method call's receiver, or the sole
/// argument of a call.
fn applied_to<'tcx>(step: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    match step.kind {
        ExprKind::MethodCall(_, receiver, ..) => Some(receiver),
        ExprKind::Call(_, [only]) => Some(only),
        _ => None,
    }
}

/// Whether `step`'s result can cross a thread boundary.
///
/// Measured on the planning file's own example:
/// `.map(|s| Rc::new(*s).len())` compiles and
/// `.map(|s| Rc::new(*s)).map(|r| r.len())` is `E0277`.
fn is_sendable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    let Some(send) = cx.tcx.get_diagnostic_item(sym::Send) else {
        return false;
    };
    implements_trait(cx, cx.typeck_results().expr_ty(step), send, &[])
}
