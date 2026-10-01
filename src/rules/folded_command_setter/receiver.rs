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
//! Reorderability is a third question, and it has two halves. The
//! receiver must not do anything the initial value could see, and the
//! initial value must not write anything the receiver reads. The first
//! half is answered over-conservatively: it asks only whether every body
//! the receiver runs is the standard library's own, instantiated with the
//! standard library's own types, which declines rewrites that are
//! provably safe, `queue.take_all()` against a plain binding among them.
//! One side is cheap to classify and two are not, and mutation reordered
//! is the kind of wrong that does not announce itself. Widening it to
//! *either* side effect-free is the obvious next step, counting an
//! initial value as effect-free when it is a place expression, a
//! literal, or a `core` / `std` call over those. The second half asks
//! whether the initial value mutates the place the receiver is rooted
//! at, or could write it through a shared reference.

use clippy_utils::res::MaybeDef;
use clippy_utils::sym;
use clippy_utils::usage::is_potentially_mutated;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CrateNum, DefId};
use rustc_hir::{Expr, ExprKind, Mutability, QPath, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty::adjustment::{Adjust, AutoBorrow, DerefAdjustKind};
use rustc_middle::ty::{self, GenericArg, Instance, Ty};
use rustc_span::{Span, Symbol};

/// `std::process::Command`'s `rustc_diagnostic_item` name, which
/// [`is_std`] uses to reach `std` itself. Not among the pre-interned
/// `rustc_span::sym` constants, so it is interned on use.
const COMMAND_DIAGNOSTIC_ITEM: &str = "Command";

/// What the suggestion needs to know about the receiver.
pub(super) struct Shape {
    /// The source whose text becomes the plural's argument. The
    /// receiver's own span where its call survives, the place alone
    /// where the call is erased.
    pub argument: Span,
    /// What method resolution did to the receiver before `fold` took
    /// it, spelled as the prefix that does it by hand.
    ///
    /// The plural's argument is a type parameter, which nothing adjusts:
    /// `it: &mut I` is reborrowed for the fold and would be moved into
    /// the plural, and a `Copy` iterator behind `&` is copied out for the
    /// fold and would be handed over as the reference.
    pub prefix: String,
    /// Whether the receiver may be evaluated after the initial value
    /// without either of them seeing the other's effects.
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
pub(super) fn shape<'tcx>(
    cx: &LateContext<'tcx>,
    receiver: &Expr<'_>,
    initial: &'tcx Expr<'tcx>,
) -> Option<Shape> {
    let prefix = prefix(cx, receiver)?;
    if is_place(receiver) {
        // Reaching a place runs no user code unless a `Deref` along it
        // does, and then that body runs before the initial value instead
        // of after it. Separately, the initial value can change what the
        // receiver reads by writing the place first.
        return Some(Shape {
            argument: receiver.span,
            prefix,
            reorderable: !derefs_through_user_code(cx, receiver)
                && !written_by(cx, receiver, initial),
        });
    }
    let ExprKind::MethodCall(_, place, [], _) = receiver.kind else {
        return None;
    };
    if !is_place(place) {
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
        prefix,
        reorderable: runs_only_std(cx, receiver, call)
            && !derefs_through_user_code(cx, place)
            && !written_by(cx, place, initial),
    })
}

/// The receiver's adjustments as the prefix that makes them by hand, or
/// `None` for an adjustment that no prefix spells.
fn prefix(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<String> {
    let mut prefix = String::new();
    for adjustment in cx.typeck_results().expr_adjustments(receiver) {
        match adjustment.kind {
            Adjust::Deref(_) => prefix.insert(0, '*'),
            Adjust::Borrow(AutoBorrow::Ref(mutability)) => {
                prefix.insert_str(0, Mutability::from(mutability).ref_prefix_str());
            }
            _ => return None,
        }
    }
    Some(prefix)
}

/// Whether evaluating `initial` may write the place `place` is rooted at.
///
/// Only a mutation can reach the receiver: a by-value receiver of a
/// `Copy` type is copied before the initial value runs, so the fold sees
/// the old value and the plural the new one. A read is harmless. Where
/// the mutation analysis cannot answer, the answer is yes.
fn written_by<'tcx>(cx: &LateContext<'tcx>, place: &Expr<'_>, initial: &'tcx Expr<'tcx>) -> bool {
    if interior_mutable(cx, place) {
        return true;
    }
    let mut root = place;
    while let ExprKind::Field(base, _) | ExprKind::Unary(UnOp::Deref, base) = root.kind {
        root = base;
    }
    let ExprKind::Path(QPath::Resolved(_, path)) = root.kind else {
        return false;
    };
    match path.res {
        Res::Local(local) => is_potentially_mutated(local, initial, cx),
        // Anything can write a `static mut`, a call the initial value
        // makes included.
        Res::Def(
            DefKind::Static {
                mutability: Mutability::Mut,
                ..
            },
            _,
        ) => true,
        _ => false,
    }
}

/// Whether the place, or anything method resolution reaches through it,
/// holds state that a shared reference can write.
///
/// Such a write takes no `&mut`, so the mutation analysis does not see
/// it, and it need not name the place at all: any alias will do.
fn interior_mutable(cx: &LateContext<'_>, place: &Expr<'_>) -> bool {
    let typeck = cx.typeck_results();
    core::iter::once(typeck.expr_ty(place))
        .chain(
            typeck
                .expr_adjustments(place)
                .iter()
                .map(|adjustment| adjustment.target),
        )
        .any(|ty| !ty.peel_refs().is_freeze(cx.tcx, cx.typing_env()))
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

/// Whether the receiver's call runs only the standard library's code.
///
/// Asked of the body the call resolves to rather than of the method it
/// names. `into_iter` names `IntoIterator`'s, but the body that runs is
/// the impl for the receiver's type, which may be the user's. And a std
/// body has to be instantiated with std types, because it can call into
/// the types it is given: cloning a `vec::IntoIter` clones each item.
fn runs_only_std(cx: &LateContext<'_>, receiver: &Expr<'_>, call: DefId) -> bool {
    let args = cx.typeck_results().node_args(receiver.hir_id);
    matches!(
        Instance::try_resolve(cx.tcx, cx.typing_env(), call, args),
        Ok(Some(instance)) if is_std(cx, instance.def_id().krate)
            && instance.args.types().all(|ty| std_throughout(cx, ty)),
    )
}

/// Whether `krate` is one of the standard library's sysroot crates.
///
/// A crate's name is not evidence. Cargo rejects a *package* called
/// `std`, but accepts one whose `[lib] name` is `alloc` and builds it,
/// and a name match then reads that crate as the standard library's.
/// The sysroot crates are reached instead through diagnostic items only
/// they carry -- `Iterator` for `core`, `Vec` for `alloc`, `Command`
/// for `std`.
fn is_std(cx: &LateContext<'_>, krate: CrateNum) -> bool {
    [
        sym::Iterator,
        sym::Vec,
        Symbol::intern(COMMAND_DIAGNOSTIC_ITEM),
    ]
    .into_iter()
    .filter_map(|item| cx.tcx.get_diagnostic_item(item))
    .any(|anchor| anchor.krate == krate)
}

/// Whether every type `ty` is built from is the standard library's own,
/// so that no code a std body reaches through it can be the user's.
///
/// A type parameter, a closure, a function pointer and a trait object
/// each stand for code this cannot see, so each counts as the user's.
fn std_throughout(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    ty.walk()
        .filter_map(GenericArg::as_type)
        .all(|ty| match ty.kind() {
            ty::Adt(def, _) => is_std(cx, def.did().krate),
            ty::Bool
            | ty::Char
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Str
            | ty::Never
            | ty::Array(..)
            | ty::Slice(_)
            | ty::Ref(..)
            | ty::RawPtr(..)
            | ty::Tuple(_) => true,
            _ => false,
        })
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

/// Whether reaching `place` runs a `Deref` body that is not the standard
/// library's own, at any step of it, written as `*` or inserted by
/// method resolution.
///
/// A `Deref` is a call, and the suggestion moves the receiver across the
/// initial value, so one that records anything would run in the other
/// order. Measured with one that counts its calls -- the fold builds
/// `ls1` and the plural `ls0`.
fn derefs_through_user_code(cx: &LateContext<'_>, place: &Expr<'_>) -> bool {
    let typeck = cx.typeck_results();
    let mut expr = place;
    loop {
        let mut source = typeck.expr_ty(expr);
        for adjustment in typeck.expr_adjustments(expr) {
            if let Adjust::Deref(DerefAdjustKind::Overloaded(_)) = adjustment.kind
                && !derefs_freely(cx, source)
            {
                return true;
            }
            source = adjustment.target;
        }
        match expr.kind {
            ExprKind::Unary(UnOp::Deref, base) => {
                // An explicit `*x` on a type with an overloaded `Deref` is
                // recorded as a method call on the unary expression itself.
                if typeck.is_method_call(expr) && !derefs_freely(cx, typeck.expr_ty(base)) {
                    return true;
                }
                expr = base;
            }
            ExprKind::Field(base, _) => expr = base,
            _ => return false,
        }
    }
}

/// Whether an overloaded `Deref` from `source` runs only the standard
/// library's code.
///
/// `Vec`, `Rc` and `Arc` deref by projecting a pointer, whatever they
/// hold. Any other type has to be the standard library's throughout,
/// because a std `Deref` can call into what it holds: `Pin`'s calls its
/// pointer's, `Cow`'s calls the owned type's `Borrow`, and `LazyLock`'s
/// runs its initializer.
fn derefs_freely(cx: &LateContext<'_>, source: Ty<'_>) -> bool {
    let source = source.peel_refs();
    [sym::Vec, sym::Rc, sym::Arc]
        .into_iter()
        .any(|item| source.is_diag_item(cx, item))
        || std_throughout(cx, source)
}
