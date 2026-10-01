//! The singular/plural pairs this rule is about, and the trait that
//! declares them.
//!
//! The set is fixed rather than configured. The suggestion is sound
//! only because upstream defines each plural as exactly the fold of its
//! singular -- `with_args` is `args.into_iter().fold(self,
//! Self::with_arg)` -- so a pair named for some other builder could not
//! promise the same.

use clippy_utils::sym;
use clippy_utils::ty::{get_iterator_item_ty, implements_trait};
use rustc_hir::Expr;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, AssocItem, GenericArg, Ty, TypeVisitableExt};
use rustc_span::Symbol;

/// What a fold over one singular setter can be replaced by.
pub(super) struct Replacement {
    /// The plural's name.
    pub plural: &'static str,
    /// Whether the plural splits each item between the singular's two
    /// parameters, which only a two-element tuple can be split between.
    pub splits_item: bool,
}

/// What replaces a fold over the singular `CommandExtra` setter
/// `singular`, or `None` for one with no plural.
///
/// `with_no_env`, `with_stdin`, `with_stdout` and `with_stderr` each set
/// one thing that a later call replaces rather than extends, so folding
/// them is a different mistake and outside this rule.
pub(super) fn replacement_for(singular: Symbol) -> Option<Replacement> {
    let (plural, splits_item) = match singular.as_str() {
        "with_arg" => ("with_args", false),
        "with_env" => ("with_envs", true),
        "without_env" => ("without_envs", false),
        _ => return None,
    };
    Some(Replacement {
        plural,
        splits_item,
    })
}

/// Whether the fold's item is a shape the plural's own bounds accept.
///
/// A plural that splits an item constrains what it will take, and which
/// constraint that is depends on the release. 1.1.0 and 1.2.0 write
/// `Envs: IntoIterator<Item = (Key, Value)>`, an associated-type
/// equality that no reference satisfies: `&[(&str, &str)]` yields
/// `&(&str, &str)`, which the *fold* takes -- `|command, (key, value)|`
/// binds through the reference -- and the plural does not. 1.3.0 and
/// later write `Envs::Item: Borrow<(Key, Value)>`, which a reference to
/// a pair does satisfy.
///
/// Reading the constraint keeps this in step with the version resolved,
/// as [`declares`] does for the plural's existence. Asking the item
/// rather than the closure's pattern is what tells the shapes apart at
/// all, since the pattern is identical either way.
pub(super) fn item_fits<'tcx>(cx: &LateContext<'tcx>, plural: DefId, receiver: &Expr<'_>) -> bool {
    let receiver_ty = cx.typeck_results().expr_ty(receiver);
    let Some(item) = get_iterator_item_ty(cx, receiver_ty) else {
        return false;
    };
    let Some(assoc_item) = iterator_item(cx) else {
        return true;
    };
    let clauses = cx.tcx.predicates_of(plural).predicates;
    // An equality is read first wherever both appear. It is the stricter
    // of the two, so answering the other one would vouch for an item the
    // equality rejects.
    for (clause, _) in clauses {
        if let Some(projection) = clause.as_projection_clause() {
            let projection = projection.skip_binder();
            if let ty::AliasTermKind::ProjectionTy { def_id } = projection.projection_term.kind
                && def_id == assoc_item
                && let Some(required) = projection.term.as_type()
                && let ty::Tuple(required) = required.kind()
            {
                return matches!(item.kind(), ty::Tuple(actual) if actual.len() == required.len());
            }
        }
    }
    for (clause, _) in clauses {
        if let Some(bound) = clause.as_trait_clause() {
            let bound = bound.skip_binder().trait_ref;
            if is_the_item(bound.self_ty(), assoc_item)
                && let Some(required) = bound.args.types().nth(1)
                && let ty::Tuple(required) = required.kind()
            {
                return satisfies(cx, plural, item, bound.def_id, required);
            }
        }
    }
    true
}

/// `IntoIterator::Item`, reached through the `into_iter` lang item's own
/// trait so that no diagnostic item has to carry it.
fn iterator_item(cx: &LateContext<'_>) -> Option<DefId> {
    let into_iter = cx.tcx.lang_items().into_iter_fn()?;
    cx.tcx
        .associated_items(cx.tcx.parent(into_iter))
        .filter_by_name_unhygienic(sym::Item)
        .map(|assoc| assoc.def_id)
        .next()
}

/// Whether `ty` is `<_ as IntoIterator>::Item`.
fn is_the_item(ty: Ty<'_>, assoc_item: DefId) -> bool {
    let ty::Alias(_, alias) = ty.kind() else {
        return false;
    };
    matches!(alias.kind, ty::AliasTyKind::Projection { def_id } if def_id == assoc_item)
}

/// Whether `item` satisfies the plural's bound on it, and the plural's
/// bounds on the tuple's own parameters.
///
/// The tuple the trait is asked about is `item` with its references
/// stripped, which is the only candidate worth trying: the plural
/// destructures a tuple, so nothing else could be split at all.
///
/// The element bounds are asked separately because the fold does not
/// prove them. Where the item is a reference to a pair the closure's
/// bindings are references too, so the fold establishes
/// `&Key: AsRef<OsStr>` where the plural asks for `Key: AsRef<OsStr>`,
/// which does not follow. Measured: a fold over `&[(Key, Value)]` whose
/// elements are `AsRef<OsStr>` only through a reference compiles, and
/// the plural over that same iterator is `E0277` on both elements.
fn satisfies<'tcx>(
    cx: &LateContext<'tcx>,
    plural: DefId,
    item: Ty<'tcx>,
    bound: DefId,
    parameters: &'tcx ty::List<Ty<'tcx>>,
) -> bool {
    let candidate = item.peel_refs();
    let ty::Tuple(elements) = candidate.kind() else {
        return false;
    };
    if elements.len() != parameters.len() {
        return false;
    }
    // `implements_trait` asserts its argument count against the trait's
    // own generics, so a bound over anything but one type parameter
    // would abort the driver rather than answer.
    if cx.tcx.generics_of(bound).count() != 2 {
        return false;
    }
    implements_trait(cx, item, bound, &[candidate.into()])
        && element_bounds_hold(cx, plural, parameters, elements)
}

/// Whether the plural's clauses on the tuple's own parameters hold for
/// the item's element types.
fn element_bounds_hold<'tcx>(
    cx: &LateContext<'tcx>,
    plural: DefId,
    parameters: &'tcx ty::List<Ty<'tcx>>,
    elements: &'tcx ty::List<Ty<'tcx>>,
) -> bool {
    for (clause, _) in cx.tcx.predicates_of(plural).predicates {
        let Some(bound) = clause.as_trait_clause() else {
            continue;
        };
        let bound = bound.skip_binder().trait_ref;
        let Some(index) = parameters
            .iter()
            .position(|parameter| parameter == bound.self_ty())
        else {
            continue;
        };
        let arguments: Vec<GenericArg<'tcx>> =
            bound.args.types().skip(1).map(GenericArg::from).collect();
        // A bound naming another of the plural's own parameters is one
        // this cannot instantiate, so it is not one to vouch for.
        if arguments
            .iter()
            .any(|argument| argument.as_type().is_some_and(|ty| ty.has_param()))
            || cx.tcx.generics_of(bound.def_id).count() != arguments.len() + 1
        {
            return false;
        }
        if !implements_trait(cx, elements[index], bound.def_id, &arguments) {
            return false;
        }
    }
    true
}

/// Whether the accumulator's own type has an inherent method named
/// `plural`, which method resolution prefers over the trait's.
///
/// The suggestion names the plural rather than resolving it, so where
/// such a method exists the rewritten call reaches that one instead.
/// Measured: a local type implementing the trait *and* carrying an
/// inherent `with_args(self, count: usize)` earned a machine-applicable
/// fix that is `E0308`.
///
/// Only the accumulator's own inherent impls are asked. A second *trait*
/// declaring the same name is ambiguity rather than shadowing, which
/// [`another_trait_declaring`] answers.
pub(super) fn shadowed_by_an_inherent_method(
    cx: &LateContext<'_>,
    initial: &Expr<'_>,
    plural: &str,
) -> bool {
    let Some(accumulator) = cx.typeck_results().expr_ty(initial).ty_adt_def() else {
        return false;
    };
    let plural = Symbol::intern(plural);
    cx.tcx
        .inherent_impls(accumulator.did())
        .iter()
        .any(|impl_id| {
            cx.tcx
                .associated_items(*impl_id)
                .filter_by_name_unhygienic(plural)
                .any(AssocItem::is_method)
        })
}

/// Whether an impl of the plural's trait that could apply to
/// `accumulator` writes its own body for the plural.
///
/// The case for the rewrite is that the trait's default plural is the
/// fold of its singular. An override keeps the trait's signature, so the
/// rewritten call still compiles, but it runs that body instead, which is
/// why this withholds the fix rather than the diagnostic.
pub(super) fn overrides_the_plural<'tcx>(
    cx: &LateContext<'tcx>,
    plural: DefId,
    accumulator: Ty<'tcx>,
) -> bool {
    let mut overrides = false;
    cx.tcx
        .for_each_relevant_impl(cx.tcx.parent(plural), accumulator, |impl_id| {
            overrides |= cx
                .tcx
                .associated_items(impl_id)
                .in_definition_order()
                .any(|item| item.trait_item_def_id() == Some(plural));
        });
    overrides
}

/// Another trait implemented for `accumulator` that declares a method
/// named like the plural, which makes the rewritten call ambiguous
/// wherever both traits are in scope.
///
/// Every visible trait is asked rather than only the call site's
/// imports, so one that is not in scope there reads as present too. That
/// errs toward withholding the fix, where the opposite error is `E0034`.
pub(super) fn another_trait_declaring<'tcx>(
    cx: &LateContext<'tcx>,
    plural: DefId,
    accumulator: Ty<'tcx>,
) -> Option<DefId> {
    let own = cx.tcx.parent(plural);
    let name = cx.tcx.item_name(plural);
    cx.tcx.visible_traits().find(|&other| {
        other != own
            && cx
                .tcx
                .associated_items(other)
                .filter_by_name_unhygienic(name)
                .any(AssocItem::is_method)
            // A trait with parameters of its own cannot be asked about
            // without them, so it is assumed to apply.
            && (cx.tcx.generics_of(other).count() != 1
                || implements_trait(cx, accumulator, other, &[]))
    })
}

pub(super) fn is_command_extra(cx: &LateContext<'_>, trait_id: DefId) -> bool {
    crate::command_extra::is_the_trait(cx, trait_id)
}

/// Whether the `CommandExtra` this build resolved declares a method
/// named `plural`.
///
/// The pairs arrived over several releases -- `without_envs` is 1.2.0
/// and later -- so a crate can have the singular without its plural,
/// which is the very reason its author wrote the fold. Asking the trait
/// carries no version table, so a plural dropped or renamed later is
/// covered by the same question.
pub(super) fn declares(cx: &LateContext<'_>, trait_id: DefId, plural: &str) -> Option<DefId> {
    cx.tcx
        .associated_items(trait_id)
        .filter_by_name_unhygienic(Symbol::intern(plural))
        .find(|assoc| AssocItem::is_method(assoc))
        .map(|assoc| assoc.def_id)
}
