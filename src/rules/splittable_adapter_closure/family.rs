//! Which family a receiver belongs to, which is what tells
//! `Iterator::map` from `Option::map`, and `Iterator::filter` from
//! `Option::filter`.
//!
//! The method names are shared across families and with other traits'
//! own adapters, whose items need not enter by value, so what the call
//! resolves to is what identifies it. Every trigger asks this question,
//! and each answers a different subset of the families.

use clippy_utils::paths::{PathNS, lookup_path};
use clippy_utils::sym;
use rustc_hir::Expr;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// One family of receiver, named for the trait or type whose adapters it
/// holds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    /// `Iterator` or `DoubleEndedIterator`.
    Iterator,
    /// `Option`.
    Option,
    /// `Result`.
    Result,
    /// `Poll`.
    Poll,
    /// `ControlFlow`.
    ControlFlow,
    /// The `Itertools` blanket extension of `Iterator`.
    Itertools,
    /// rayon's `ParallelIterator`.
    Rayon,
    /// `pipe-trait`'s `Pipe`.
    Pipe,
}

impl Family {
    /// Whether a lifted step's result has to be `Send`.
    ///
    /// `ParallelIterator::map` requires it of the item it produces,
    /// where the folded form does not, because the value never leaves
    /// the closure. `.map(|s| Rc::new(*s).len())` compiles and
    /// `.map(|s| Rc::new(*s)).map(|r| r.len())` does not.
    pub(super) fn sends_between_threads(self) -> bool {
        self == Self::Rayon
    }
}

/// Which family `receiver` belongs to, or `None` where the call is none
/// of the rule's.
///
/// `rfold` and `try_rfold` are `DoubleEndedIterator`'s, the same shape
/// worked from the other end; they lift into `Iterator::map` all the
/// same, because `Map` is double-ended wherever its iterator is.
pub(super) fn family<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Expr<'tcx>,
    receiver: &'tcx Expr<'tcx>,
) -> Option<Family> {
    let method = cx.typeck_results().type_dependent_def_id(call.hir_id)?;
    if let Some(declaring) = cx.tcx.trait_of_assoc(method) {
        let declares = |name| cx.tcx.is_diagnostic_item(name, declaring);
        if declares(sym::Iterator) || declares(sym::DoubleEndedIterator) {
            return Some(Family::Iterator);
        }
        // A trait from a crate the linted one does not depend on resolves
        // to nothing, so these cost a lookup and answer `None` there.
        return extension(cx, declaring);
    }
    let adt = cx
        .typeck_results()
        .expr_ty_adjusted(receiver)
        .peel_refs()
        .ty_adt_def()?;
    let carries = |name| cx.tcx.is_diagnostic_item(name, adt.did());
    let named = [
        (sym::Option, Family::Option),
        (sym::Result, Family::Result),
        (sym::ControlFlow, Family::ControlFlow),
    ]
    .into_iter()
    .find_map(|(name, family)| carries(name).then_some(family));
    // `Poll` carries no diagnostic item, measured by asking
    // `get_diagnostic_name` for it, so the path is what identifies it.
    named.or_else(|| {
        let poll = lookup_path(
            cx.tcx,
            PathNS::Type,
            &[sym::core, Symbol::intern("task"), sym::Poll],
        );
        poll.contains(&adt.did()).then_some(Family::Poll)
    })
}

/// Which extension trait `declaring` is, or `None` for a trait the rule
/// does not speak about.
///
/// None of these carries a diagnostic item, so the path is what
/// identifies it. `Itertools` is a blanket extension of `Iterator` and
/// `Pipe` one of everything, which is why a receiver's type says nothing
/// about either.
fn extension(cx: &LateContext<'_>, declaring: DefId) -> Option<Family> {
    let intern = Symbol::intern;
    let paths = [
        (
            vec![intern("itertools"), intern("Itertools")],
            Family::Itertools,
        ),
        (
            vec![intern("rayon"), intern("iter"), intern("ParallelIterator")],
            Family::Rayon,
        ),
        // The positional adapters are declared on the subtrait, so a
        // lookup of the base one alone never reaches them.
        (
            vec![
                intern("rayon"),
                intern("iter"),
                intern("IndexedParallelIterator"),
            ],
            Family::Rayon,
        ),
        (vec![intern("pipe_trait"), intern("Pipe")], Family::Pipe),
    ];
    paths.into_iter().find_map(|(path, family)| {
        let found = lookup_path(cx.tcx, PathNS::Type, &path);
        found.contains(&declaring).then_some(family)
    })
}
