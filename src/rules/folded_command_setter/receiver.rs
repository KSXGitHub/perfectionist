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
//! Erasing the receiver's call is the opposite kind of question, and is
//! answered by what the call resolves to. A local `iter` that disagrees
//! with its own `IntoIterator` impl still compiles after the erasure and
//! silently yields a different order, so the name is no evidence.
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

use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, QPath, UnOp};
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

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
    if is_place(receiver) {
        // Nothing to evaluate, so nothing the initial value could
        // observe out of order.
        return Some(Shape {
            argument: receiver.span,
            reorderable: true,
        });
    }
    let ExprKind::MethodCall(segment, place, [], _) = receiver.kind else {
        return None;
    };
    if !is_place(place) {
        return None;
    }
    let call = cx.typeck_results().type_dependent_def_id(receiver.hir_id)?;
    let argument = if erases(cx, segment.ident.name, call, place) {
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
fn erases(cx: &LateContext<'_>, name: Symbol, call: DefId, place: &Expr<'_>) -> bool {
    if cx.tcx.lang_items().into_iter_fn() == Some(call) {
        // The very function the plural calls, so the two agree by
        // construction. Compared by `DefId` because an *inherent*
        // `into_iter` shadows the trait in method resolution and need
        // not agree with it.
        return true;
    }
    // There is no `Iterator::iter` to compare against -- every `iter` is
    // an inherent method of its own type, and `Vec`'s is `<[T]>::iter`
    // reached through `Deref` -- so the only guarantee available is
    // std's convention that `&C: IntoIterator` agrees with `C::iter()`.
    // A local `iter` promises nothing.
    name == Symbol::intern("iter")
        && from_std(cx, call)
        // On an owned collection the erasure moves what the fold merely
        // borrowed, and the code around it stops compiling.
        && cx.typeck_results().expr_ty(place).is_ref()
}

/// Whether a call resolves into the standard library.
fn from_std(cx: &LateContext<'_>, call: DefId) -> bool {
    matches!(
        cx.tcx.crate_name(call.krate).as_str(),
        "core" | "std" | "alloc",
    )
}

/// Whether `expr` names a location rather than computing one.
///
/// Narrower than Rust's own notion of a place expression, which counts
/// every indexing expression however much its index computes. What this
/// gate is for is keeping the relocated text short, and `list[seek()]`
/// is not that.
fn is_place(expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(..) | QPath::TypeRelative(..)) => true,
        ExprKind::Field(base, _) | ExprKind::Unary(UnOp::Deref, base) => is_place(base),
        _ => false,
    }
}
