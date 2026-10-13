//! What the method probe would find at a call whose receiver the
//! rewrite has made owned.

use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// Whether the trait methods in scope at `call` include one of this
/// name.
///
/// The chain's trailing call keeps its name while its receiver becomes
/// an owned `Command`, which moves the method probe's first step from
/// `&mut Command` to `Command`. Against a `&mut Command`,
/// `Command::status(&mut self)` matches at step 0 *by value*, because
/// the step type already is the `&mut Command` its receiver wants --
/// the first pick the probe tries, which nothing can get ahead of.
/// Against a `Command` it matches nothing until the `&mut` autoref, the
/// *last* of the three picks at that step, so a candidate taking `self`
/// or `&self` is found first. Only a trait can supply such a candidate,
/// since nobody outside the standard library can write an inherent impl
/// for `Command`. Two of them would be `E0034` and stop the fixer with
/// an error; exactly one compiles and silently calls something else.
///
/// `in_scope_traits` is the set the method probe itself consults, so
/// no trait is weighed that resolution would not weigh. The guard
/// deliberately does not ask these, each because the safe answer is the
/// one that declines:
///
/// - Whether `Command` implements the trait. Answering means naming
///   the trait's other generic arguments, and where `Command` does not
///   pin them -- `Into` and `TryInto` are the everyday cases -- the
///   answer comes back "no" for want of an inference.
/// - What the method's receiver is. `self` and `&self` both reach the
///   owned command ahead of the inherent setter, and `&mut self` ties
///   with it and loses. Declining on the receiver shape alone would be
///   unsound, though, because an inherent `&self` method loses to a
///   trait `&mut self` one before the change and wins after. The sound
///   refinement is narrower -- no inherent method of the name, and
///   every candidate taking `&mut self` -- and what it would buy is
///   `CommandExt::exec` and its neighbours, which this costs today.
///
/// `is_method` is what keeps an associated function out, since its
/// `has_self` is exactly the set `value.name()` can reach: a
/// `fn status(this: Self)` is excluded and an arbitrary self type is
/// not.
pub(super) fn finds_a_trait_method(cx: &LateContext<'_>, call: &Expr<'_>, name: Symbol) -> bool {
    cx.tcx
        .in_scope_traits(call.hir_id)
        .unwrap_or_default()
        .iter()
        .any(|candidate| {
            cx.tcx
                .associated_items(candidate.def_id)
                .filter_by_name_unhygienic(name)
                .any(rustc_middle::ty::AssocItem::is_method)
        })
}
