//! What the fold's receiver has to look like, and what of it survives
//! into the suggestion.
//!
//! The shape gate is a value judgement rather than a correctness one.
//! The rewrite stays valid for any receiver, because the receiver only
//! moves; it stops being an *improvement* once what moves is long or
//! carries logic of its own. So what the gate measures is the written
//! form -- how much text the suggestion would relocate -- which is why
//! it is syntactic where the folder's check is not.
//!
//! Erasing the receiver's call is the opposite kind of question, and one
//! the method's name cannot answer. Method resolution autorefs and
//! autoderefs, so the place the call is written on need not be the
//! receiver the call ran on, and handing the plural that place instead
//! hands it something else: `&&Vec<T>`, which is not an iterator at all,
//! or a collection whose only `IntoIterator` is on `&Self`. Worse, a
//! `Deref` is enough to hand `iter` to *std* while the type keeps an
//! `IntoIterator for &Self` of its own -- measured reversing a command's
//! arguments under a fix the rule called machine-applicable. So only
//! `IntoIterator::into_iter` on the place's own type is erased, because
//! that is the one case where the plural calls the same function on the
//! same value.
//!
//! Reorderability is a third question, and the answer here is
//! deliberately over-conservative: it asks only where the receiver's
//! call came from, which declines rewrites that are provably safe --
//! `queue.take_all()` against a plain binding among them. One side is
//! cheap to classify and two are not, and mutation reordered is the kind
//! of wrong that does not announce itself. Widening it to *either* side
//! effect-free is the obvious next step, counting an initial value as
//! effect-free when it is a place expression, a literal, or a `core` /
//! `std` call over those.

use clippy_utils::sym;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, QPath, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty::adjustment::{Adjust, DerefAdjustKind};
use rustc_span::{Span, Symbol};

/// `std::process::Command`'s `rustc_diagnostic_item` name, which
/// [`from_std`] uses to reach `std` itself. Not among the pre-interned
/// `rustc_span::sym` constants, so it is interned on use.
const COMMAND_DIAGNOSTIC_ITEM: &str = "Command";

/// What the suggestion needs to know about the receiver.
pub(super) struct Shape {
    /// The source whose text becomes the plural's argument. The
    /// receiver's own span where its call survives, the place alone
    /// where the call is erased.
    pub argument: Span,
    /// Whether the receiver may be evaluated after the initial value
    /// without changing what the initial value sees.
    ///
    /// The suggestion trades the two, so it trades the order they run
    /// in: `A.fold(B, f)` evaluates `A` then `B`, and `B.plural(A)`
    /// evaluates `B` then `A`.
    pub reorderable: bool,
}

/// The receiver's shape, or `None` where it is not a simple iterator
/// expression: a place expression followed by at most one argument-less
/// method call.
///
/// An argument is where logic hides, and a second call is more text
/// moving. Both bars are structural rather than a list of adapter names
/// that would need extending as the iterator API grows.
pub(super) fn shape(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<Shape> {
    if is_place(cx, receiver) {
        // Reaching a place runs no user code, so there is nothing the
        // initial value could observe out of order.
        return Some(Shape {
            argument: receiver.span,
            reorderable: true,
        });
    }
    let ExprKind::MethodCall(_, place, [], _) = receiver.kind else {
        return None;
    };
    if !is_place(cx, place) {
        return None;
    }
    let call = cx.typeck_results().type_dependent_def_id(receiver.hir_id)?;
    let argument = if erases(cx, call, place) {
        place.span
    } else {
        receiver.span
    };
    Some(Shape {
        argument,
        reorderable: from_std(cx, call),
    })
}

/// Whether the receiver's call can be dropped from the suggestion
/// because the plural performs it itself.
///
/// Only `IntoIterator::into_iter` on the place's own type qualifies. The
/// plural's body calls that very function on the value it is handed, so
/// erasing it substitutes a call for itself. Compared by `DefId` because
/// an *inherent* `into_iter` shadows the trait in method resolution and
/// need not agree with it, and guarded on the place carrying no
/// adjustment because an autoref or autoderef means the call ran on
/// something the place is not.
fn erases(cx: &LateContext<'_>, call: DefId, place: &Expr<'_>) -> bool {
    cx.tcx.lang_items().into_iter_fn() == Some(call)
        && cx.typeck_results().expr_adjustments(place).is_empty()
}

/// Whether a call resolves into the standard library, asked of the
/// sysroot crates themselves rather than of a name.
///
/// A crate's name is not evidence. Cargo rejects a *package* called
/// `std`, but accepts one whose `[lib] name` is `alloc` and builds it,
/// and a name match then reads that crate as the standard library's.
/// The sysroot crates are reached instead through diagnostic items only
/// they carry -- `Iterator` for `core`, `Vec` for `alloc`, `Command`
/// for `std`.
fn from_std(cx: &LateContext<'_>, call: DefId) -> bool {
    [
        sym::Iterator,
        sym::Vec,
        Symbol::intern(COMMAND_DIAGNOSTIC_ITEM),
    ]
    .into_iter()
    .filter_map(|item| cx.tcx.get_diagnostic_item(item))
    .any(|anchor| anchor.krate == call.krate)
}

/// Whether `expr` names a location rather than computing one.
///
/// Narrower than Rust's own notion of a place expression, which counts
/// every indexing expression however much its index computes. What this
/// gate is for is keeping the relocated text short, and `list[seek()]`
/// is not that.
///
/// An overloaded `Deref` is excluded for a second reason: it is a call,
/// and the suggestion moves the receiver across the initial value, so a
/// `Deref` that records anything would run in the other order. Measured
/// with one that counts its calls -- the fold builds `ls1` and the
/// plural `ls0`.
fn is_place(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(..) | QPath::TypeRelative(..)) => true,
        ExprKind::Field(base, _) | ExprKind::Unary(UnOp::Deref, base) => {
            is_place(cx, base) && reached_freely(cx, expr, base)
        }
        _ => false,
    }
}

/// Whether reaching through `base` to `expr` runs no user code.
fn reached_freely(cx: &LateContext<'_>, expr: &Expr<'_>, base: &Expr<'_>) -> bool {
    // An explicit `*x` on a type with an overloaded `Deref` is recorded
    // as a method call on the unary expression itself.
    !cx.typeck_results().is_method_call(expr)
        && !cx
            .typeck_results()
            .expr_adjustments(base)
            .iter()
            .any(|adjustment| {
                matches!(
                    adjustment.kind,
                    Adjust::Deref(DerefAdjustKind::Overloaded(_)),
                )
            })
}
