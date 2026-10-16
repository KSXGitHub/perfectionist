//! Which `Option` combinator the closure ends in, and what the split
//! hands its work to.
//!
//! Each form below was tabled in the planning file with both halves run
//! and their outputs compared. The correspondence is between a
//! combinator and an iterator adapter doing the same thing, so the split
//! moves work rather than changing it.

use crate::adapter_discipline::Discipline;
use crate::binding_uses::names;
use crate::common::{binding_hir_id, binds_mutably, borrows};
use crate::exclusive_captures::exclusive;
use crate::extra_reference::survives;
use core::mem::discriminant;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ExprKind, Pat};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Symbol;

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
    Guarded,
}

impl Split {
    /// What the closure is doing twice, which the `Fallible` shape has no
    /// guard in.
    ///
    /// This speaks for a body whose chain holds one seam, so the two
    /// stages it names are the whole of that body. [`Seams::message`] is
    /// what a longer chain is reported by.
    fn summary(&self) -> &'static str {
        match self {
            Self::Fallible(_) => "runs two fallible stages",
            Self::Infallible => "makes a value after a stage that may fail",
            Self::Filtering => "tests a value it made in the same closure",
            Self::Guarded => "holds a guard and a value",
        }
    }

    /// What to tell the reader to do, which differs by what the split
    /// hands the work to.
    ///
    /// Which adapter each shape names is `discipline`'s to say, since an
    /// adapter answering by the leading run takes the work in a different
    /// one from an adapter answering by the whole set.
    ///
    /// `every` names the seam for a chain holding more than one of this
    /// shape, so following the help once leaves no seam behind. A chain
    /// mixing shapes is told [`Seams::help`]'s kind-agnostic form instead,
    /// this one naming only its own.
    fn help(&self, discipline: Discipline, every: bool) -> String {
        let seam = if every { "each" } else { "the" };
        let (stage, test) = (discipline.fallible_target(), discipline.lift_target());
        match self {
            Self::Fallible(Yield::Stream) => format!(
                "give {seam} `and_then` a trailing `{stage}` of its own, so each fallible \
                 stage is one adapter",
            ),
            Self::Fallible(Yield::OneValue) => format!(
                "give the stage before {seam} `and_then` a leading `{stage}` of its own, \
                 so each fallible stage is one adapter",
            ),
            Self::Infallible => format!(
                "give {seam} `map` its own trailing `map`, so the adapter is left only what \
                 can fail",
            ),
            Self::Filtering => format!(
                "give {seam} `filter` its own trailing `{test}`, so the adapter is left only \
                 what it makes",
            ),
            Self::Guarded => format!(
                "lift the guard into a leading `{test}` and let a `map` make the value, so \
                 neither adapter does both",
            ),
        }
    }
}

/// The seams the closure's chain holds: the shape the outermost one takes,
/// and how far the chain below it runs.
///
/// A seam is a boundary the split can hand work across, so the count is
/// what the closure would have to be rewritten into: a body of one seam
/// becomes two adapters, and each further seam one more.
pub(super) struct Seams {
    /// The shape of the seam the body ends in, which is the one every
    /// [`Split`] form is named for.
    outermost: Split,
    /// The innermost receiver plus every seam above it. A body holding one
    /// seam counts 2, which is the case each [`Split`] form's own text
    /// speaks for.
    stages: usize,
    /// Whether every seam takes `outermost`'s shape, which is what lets
    /// that shape's lift target be named for all of them at once.
    alike: bool,
    /// The discipline of the adapter the closure was passed to, which is
    /// what names each shape's lift target.
    discipline: Discipline,
}

impl Seams {
    /// What to report, naming `adapter` as the one doing the stages.
    pub(super) fn message(&self, adapter: Symbol) -> String {
        match self.stages {
            2 => format!(
                "this closure {}, so `{adapter}` does both",
                self.outermost.summary(),
            ),
            stages => format!("this closure runs {stages} stages, so `{adapter}` does all of them"),
        }
    }

    /// What to tell the reader to do, which has to leave no seam behind:
    /// a help the reader can follow and be reported again is advice that
    /// did not finish.
    pub(super) fn help(&self) -> String {
        match (self.stages, self.alike) {
            (2, _) => self.outermost.help(self.discipline, false),
            (_, true) => self.outermost.help(self.discipline, true),
            // A chain of unalike seams has a lift target per seam rather
            // than one for all of them, and naming the outermost one's
            // would leave the rest unaccounted for.
            (_, false) => {
                "give each stage an adapter of its own, so no adapter runs two".to_owned()
            }
        }
    }
}

/// The seams `body`'s chain holds under `discipline`, or `None` where it
/// holds none.
///
/// Only the seams that are themselves splittable are counted, each by the
/// same check the outermost one passes. A seam the check declines ends the
/// walk rather than being counted and left out of the help, so the help
/// promises no split that would not compile: a chain whose inner stage
/// borrows is reported as the one seam above it, not as the whole chain.
pub(super) fn split<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Expr<'tcx>,
    closure: LocalDefId,
    parameter: &'tcx Pat<'tcx>,
    discipline: Discipline,
    yields: Yield,
) -> Option<Seams> {
    let outermost = seam(cx, body, closure, parameter, yields)?;
    let (mut stages, mut alike, mut current) = (2, true, body);
    while let ExprKind::MethodCall(_, receiver, _, _) = current.kind {
        // Every seam below the outermost sits under the adapter the
        // outermost split leaves in front of it, and that one yields a
        // stream whichever way the adapter being split yields: the leading
        // adapter a one-value split calls for is a `filter_map`.
        let Some(inner) = seam(cx, receiver, closure, parameter, Yield::Stream) else {
            break;
        };
        stages += 1;
        alike &= discriminant(&inner) == discriminant(&outermost);
        current = receiver;
    }
    Some(Seams {
        outermost,
        stages,
        alike,
        discipline,
    })
}

/// The split the seam `body` ends in admits, or `None` where it admits
/// none.
///
/// The adapter's discipline does not reach here. It says which adapter the
/// lifted work is named for, which [`Split::help`] asks it, and every
/// shape splits under either discipline once that name is the right one.
///
/// The receiver has to name the item in every form: it is the stage the
/// adapter keeps, and a receiver naming no item asks the same question
/// of each one.
///
/// Where the argument may name the item differs by form. A `then`'s
/// value becomes a `map` over the item, so it may; every other form's
/// argument becomes the closure of an adapter handed the stage before
/// it, where the item is out of scope.
fn seam<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Expr<'tcx>,
    closure: LocalDefId,
    parameter: &'tcx Pat<'tcx>,
    yields: Yield,
) -> Option<Split> {
    let item = binding_hir_id(parameter)?;
    // A combinator the reader did not write has no stage boundary they
    // can move: the body they see is one macro call.
    if body.span.from_expansion() {
        return None;
    }
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
        "then" | "then_some" if receiver_ty.is_bool() => Split::Guarded,
        "and_then" if is_option(cx, receiver_ty) => Split::Fallible(yields),
        "map" if is_option(cx, receiver_ty) => Split::Infallible,
        "filter" if is_option(cx, receiver_ty) => Split::Filtering,
        _ => return None,
    };
    if !matches!(split, Split::Guarded) && names(cx, argument, &[item]) {
        return None;
    }
    // Each half of the split gets a closure of its own, and a capture only
    // a shared borrow or a copy of a `Copy` value lets both of them hold,
    // so any other one both halves reach does not compile once split.
    let held_alone = exclusive(cx, closure);
    if names(cx, receiver, &held_alone) && names(cx, argument, &held_alone) {
        return None;
    }
    // `Fallible` and `Infallible` move the later stage into an adapter of
    // its own, so the earlier stage's `Option` becomes that adapter's item
    // and leaves the closure. A result borrowing anything is `E0515`
    // there, and the regions are erased by now, so a borrow of the item
    // cannot be told from one of something outliving the closure.
    if matches!(split, Split::Fallible(_) | Split::Infallible) && borrows(receiver_ty) {
        return None;
    }
    // A guard lifts into `filter` or `take_while`, which hand the item by
    // reference where `filter_map` and its kin hand it over. A guard that
    // moves the item is then `E0308` and one that writes to it `E0596`,
    // with nothing to amend by hand.
    if matches!(split, Split::Guarded) && !takes_one_more_reference(cx, parameter, receiver) {
        return None;
    }
    // A one-value adapter has no trailing adapter but the `Option`'s, so
    // the two forms lifting work to its right go somewhere other than
    // where they read. `Filtering`'s trailing `filter` becomes
    // `Option::filter`, which rejects what the search settled on;
    // `Guarded`'s `map` would have to follow an iterator the guard's
    // leading adapter produced, and the one value is gone.
    match (yields, &split) {
        (Yield::OneValue, Split::Filtering | Split::Guarded) => None,
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
/// The guard moves into `filter` or `take_while`, which hand `&Item`. Two
/// things do not survive that, and neither is visible in the other: an
/// item the guard writes to has nothing to write through, which the
/// binding and the type answer for, and a mention the extra reference
/// changes the meaning of, which [`crate::extra_reference`] answers for.
fn takes_one_more_reference<'tcx>(
    cx: &LateContext<'tcx>,
    parameter: &'tcx Pat<'tcx>,
    guard: &'tcx Expr<'tcx>,
) -> bool {
    let item_ty = cx.typeck_results().pat_ty(parameter);
    if item_ty.is_mutable_ptr() || binds_mutably(parameter) {
        return false;
    }
    binding_hir_id(parameter).is_some_and(|item| survives(cx, guard, item, item_ty))
}
