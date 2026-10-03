//! Which `Option` combinator the closure ends in, and what the split
//! hands its work to.
//!
//! Each form below was tabled in the planning file with both halves run
//! and their outputs compared. The correspondence is between a
//! combinator and an iterator adapter doing the same thing, so the split
//! moves work rather than changing it.

use crate::adapter_discipline::Discipline;
use crate::binding_uses::names;
use crate::exclusive_captures::exclusive;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ExprKind, HirId};
use rustc_lint::LateContext;
use rustc_middle::ty;

/// Whether the adapter yields a stream or one value.
///
/// A one-value adapter's trailing adapter is the `Option`'s own rather
/// than the iterator's, so a form lifting work to the right of it hands
/// that work somewhere else than it reads.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Yield {
    Stream,
    OneValue,
}

/// The shape the closure ends in, named for what the split hands the
/// work to.
pub(super) enum Split {
    /// One of the two fallible stages becomes a `filter_map` of its own.
    /// Which one depends on the yield: a stream's trailing adapter is an
    /// iterator's, so the `and_then` takes it, while a one-value
    /// adapter's is the `Option`'s, so the stage before the `and_then`
    /// takes a leading one instead.
    Fallible(Yield),

    /// `X.map(f)` becomes a trailing `map`, which filters nothing and so
    /// suits either discipline.
    Infallible,
    /// `X.filter(p)` becomes a trailing `filter`, which filters the way
    /// a set-shaped adapter does.
    Filtering,
    /// `guard.then(|| value)` splits in two: the guard into a leading
    /// adapter of the outer one's discipline, the value into a `map`.
    Guarded(Discipline),
}

impl Split {
    /// What to tell the reader to do, which differs by what the split
    /// hands the work to.
    pub(super) fn help(&self) -> String {
        match self {
            Self::Fallible(Yield::Stream) => {
                "give the `and_then` a trailing `filter_map` of its own, so each fallible \
                 stage is one adapter"
                    .to_owned()
            }
            Self::Fallible(Yield::OneValue) => {
                "give the stage before the `and_then` a leading `filter_map` of its own, so \
                 each fallible stage is one adapter"
                    .to_owned()
            }
            Self::Infallible => {
                "give the `map` its own trailing `map`, so the adapter is left only what \
                 can fail"
                    .to_owned()
            }
            Self::Filtering => {
                "give the `filter` its own trailing `filter`, so the adapter is left only \
                 what it makes"
                    .to_owned()
            }
            Self::Guarded(discipline) => format!(
                "lift the guard into a leading `{}` and let a `map` make the value, so \
                 neither adapter does both",
                discipline.lift_target(),
            ),
        }
    }
}

/// The split `body` admits under `discipline`, or `None` where it admits
/// none.
///
/// The receiver has to name the item in every form: it is the stage the
/// adapter keeps, and a receiver naming no item asks the same question
/// of each one.
///
/// Where the argument may name the item differs by form. A `then`'s
/// value becomes a `map` over the item, so it may; every other form's
/// argument becomes the closure of an adapter handed the stage before
/// it, where the item is out of scope.
pub(super) fn split<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Expr<'tcx>,
    closure: LocalDefId,
    item: HirId,
    item_ty: ty::Ty<'tcx>,
    discipline: Discipline,
    yields: Yield,
) -> Option<Split> {
    let ExprKind::MethodCall(segment, receiver, arguments, _) = body.kind else {
        return None;
    };
    let [argument] = arguments else {
        return None;
    };
    if !names(cx, receiver, &[item]) {
        return None;
    }
    let receiver_ty = cx.typeck_results().expr_ty(receiver);
    let split = match segment.ident.name.as_str() {
        "then" | "then_some" if receiver_ty.is_bool() => Split::Guarded(discipline),
        "and_then" if is_option(cx, receiver_ty) => Split::Fallible(yields),
        "map" if is_option(cx, receiver_ty) => Split::Infallible,
        "filter" if is_option(cx, receiver_ty) => Split::Filtering,
        _ => return None,
    };
    if !matches!(split, Split::Guarded(_)) && names(cx, argument, &[item]) {
        return None;
    }
    // Each half of the split gets a closure of its own, and two closures
    // cannot both hold a capture held any way but shared, so a capture
    // both halves reach does not compile once split.
    let held_alone = exclusive(cx, closure);
    if names(cx, receiver, &held_alone) && names(cx, argument, &held_alone) {
        return None;
    }
    // A leading `filter_map` or a trailing `filter` drops items, which
    // moves where a prefix-shaped adapter stops. Only the forms lifting
    // into a `take_while`, or into a `map` that drops nothing, survive
    // there.
    if let (Discipline::Prefix, Split::Fallible(_) | Split::Filtering) = (discipline, &split) {
        return None;
    }
    // A guard lifts into `filter` or `take_while`, which hand the item by
    // reference where `filter_map` and its kin hand it over. A guard that
    // moves the item is then `E0308` and one that needs it mutably
    // `E0596`, with nothing to amend by hand. A shared reference and a
    // `Copy` item are the two that survive the extra reference.
    if matches!(split, Split::Guarded(_)) && !takes_one_more_reference(cx, item_ty) {
        return None;
    }
    // A one-value adapter has no trailing adapter but the `Option`'s, so
    // the two forms lifting work to its right go somewhere other than
    // where they read. `Filtering`'s trailing `filter` becomes
    // `Option::filter`, which rejects what the search settled on;
    // `Guarded`'s `map` would have to follow an iterator the guard's
    // leading adapter produced, and the one value is gone.
    match (yields, &split) {
        (Yield::OneValue, Split::Filtering | Split::Guarded(_)) => None,
        _ => Some(split),
    }
}

/// Whether `ty` is an `Option`, which is what tells `Option::map` from
/// every other `map`.
fn is_option<'tcx>(cx: &LateContext<'tcx>, ty: ty::Ty<'tcx>) -> bool {
    ty.ty_adt_def().is_some_and(|adt| {
        cx.tcx
            .is_diagnostic_item(rustc_span::sym::Option, adt.did())
    })
}

/// Whether the item survives being handed one reference more than it was.
///
/// A shared reference derefs through the extra one, and a `Copy` item is
/// read through it. An owned item that the guard moves, and a mutable
/// reference it writes through, do neither.
fn takes_one_more_reference<'tcx>(cx: &LateContext<'tcx>, item_ty: ty::Ty<'tcx>) -> bool {
    if item_ty.is_mutable_ptr() {
        return false;
    }
    item_ty.is_ref() || cx.type_is_copy_modulo_regions(item_ty)
}
