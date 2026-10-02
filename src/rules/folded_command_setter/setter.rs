//! The singular/plural pairs this rule is about, and the trait that
//! declares them.
//!
//! The set is fixed rather than configured. The suggestion is sound
//! only because upstream defines each plural as exactly the fold of its
//! singular -- `with_args` is `args.into_iter().fold(self,
//! Self::with_arg)` -- so a pair named for some other builder could not
//! promise the same.

use clippy_utils::sym;
use clippy_utils::ty::{deref_chain, get_iterator_item_ty, implements_trait};
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, LangItem};
use rustc_infer::infer::TyCtxtInferExt;
use rustc_lint::LateContext;
use rustc_middle::traits::EvaluationResult;
use rustc_middle::ty::fast_reject::DeepRejectCtxt;
use rustc_middle::ty::{self, AssocItem, GenericArg, GenericArgs, TraitRef, Ty, TypeVisitableExt};
use rustc_span::{DUMMY_SP, Symbol};
use rustc_trait_selection::traits::query::evaluate_obligation::InferCtxtExt as _;
use rustc_trait_selection::traits::{Obligation, ObligationCause};

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
/// all, since the pattern is identical either way. A constraint in
/// neither form answers no, because nothing then says which items the
/// plural accepts.
pub(super) fn item_fits<'tcx>(cx: &LateContext<'tcx>, plural: DefId, receiver: &Expr<'_>) -> bool {
    // Adjusted, because the item is what `fold` iterates: a `Copy`
    // iterator behind `&` is copied out first.
    let receiver_ty = cx.typeck_results().expr_ty_adjusted(receiver);
    let Some(item) = get_iterator_item_ty(cx, receiver_ty) else {
        return false;
    };
    let Some(assoc_item) = iterator_item(cx) else {
        return false;
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
    false
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
    // own generics. `bound` takes one type parameter besides `Self`,
    // because the rule reads no plural with any other bound on its item.
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

/// Whether an inherent method named like the plural takes the
/// accumulator by value, which method resolution prefers over the
/// trait's.
///
/// The suggestion names the plural rather than resolving it, so where
/// such a method exists the rewritten call reaches that one instead.
/// Measured: a local type implementing the trait *and* carrying an
/// inherent `with_args(self, count: usize)` earned a machine-applicable
/// fix that is `E0308`.
///
/// Method probing gathers inherent methods from every type the
/// accumulator derefs to, and at its first step takes any whose receiver
/// is the accumulator itself. So each is asked whether its receiver could
/// be the accumulator: `fn with_args(self: Box<Self>)` on `U` could be a
/// `Box<U>`, and neither a method taking `&self` nor one in
/// `impl Wrapper<u32>` could be a `Wrapper<Command>`.
///
/// Only inherent methods are asked. A second *trait* declaring the same
/// name is ambiguity rather than shadowing, which
/// [`another_trait_declaring`] answers.
pub(super) fn shadowed_by_an_inherent_method(
    cx: &LateContext<'_>,
    initial: &Expr<'_>,
    plural: DefId,
) -> bool {
    let accumulator = cx.typeck_results().expr_ty(initial);
    let name = cx.tcx.item_name(plural);
    let reject = DeepRejectCtxt::relate_rigid_infer(cx.tcx);
    autoderef_steps(cx, accumulator)
        .filter_map(Ty::ty_adt_def)
        .flat_map(|adt| cx.tcx.inherent_impls(adt.did()))
        .flat_map(|impl_id| {
            cx.tcx
                .associated_items(*impl_id)
                .filter_by_name_unhygienic(name)
        })
        .filter(|item| item.is_method())
        .filter_map(|item| {
            cx.tcx
                .fn_sig(item.def_id)
                .skip_binder()
                .inputs()
                .skip_binder()
                .first()
                .copied()
        })
        .any(|receiver| reject.types_may_unify(accumulator, receiver))
}

/// The types method probing steps through from `ty`, bounded as rustc's
/// own autoderef is: a `Deref` whose target leads back round would
/// otherwise never end, and measured, the lint did not.
fn autoderef_steps<'cx, 'tcx>(
    cx: &'cx LateContext<'tcx>,
    ty: Ty<'tcx>,
) -> impl Iterator<Item = Ty<'tcx>> + 'cx {
    deref_chain(cx, ty).take(cx.tcx.recursion_limit().0)
}

/// Whether an impl of the plural's trait that could apply to
/// `accumulator` writes its own body for the plural.
///
/// The case for the rewrite is that the trait's default plural is the
/// fold of its singular. An override keeps the trait's signature, so the
/// rewritten call still compiles, but it runs that body instead, which is
/// why this withholds the fix rather than the diagnostic.
///
/// For a type parameter or an alias, `for_each_relevant_impl` visits
/// only the blanket impls, though either can stand for a type whose own
/// impl overrides the plural. So the answer for one is yes.
pub(super) fn overrides_the_plural<'tcx>(
    cx: &LateContext<'tcx>,
    plural: DefId,
    accumulator: Ty<'tcx>,
) -> bool {
    if let ty::Param(_) | ty::Alias(..) = accumulator.kind() {
        return true;
    }
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

/// Whether `expr` sits in the body of the plural itself, closures within
/// it included, where the suggestion could make the plural call itself.
///
/// In the trait's default body every fold counts, because any
/// accumulator whose plural is that default reaches it again. In an
/// impl's override only a fold over the impl's own type does: one over
/// another type calls that type's plural, as a wrapper delegating to its
/// inner command does.
pub(super) fn inside_the_plural(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    initial: &Expr<'_>,
    plural: DefId,
) -> bool {
    let owner = cx
        .tcx
        .typeck_root_def_id(cx.tcx.hir_enclosing_body_owner(expr.hir_id).to_def_id());
    if owner == plural {
        return true;
    }
    cx.tcx
        .opt_associated_item(owner)
        .and_then(|item| item.trait_item_def_id())
        == Some(plural)
        // Typeck erases the regions of the types it records, so the
        // impl's own type is compared with its regions erased too.
        && cx.typeck_results().expr_ty(initial)
            == cx.tcx.erase_and_anonymize_regions(
                cx.tcx
                    .type_of(cx.tcx.parent(owner))
                    .instantiate_identity()
                    .skip_normalization(),
            )
}

/// Another trait that declares a method named like the plural which
/// method probing could pick for `accumulator`, which makes the rewritten
/// call ambiguous wherever both traits are in scope.
///
/// Probing asks, at its first step, for a method whose receiver is the
/// accumulator itself, from whichever type along the deref chain the
/// trait is implemented for. So `fn without_envs(self)` competes for a
/// `Command`, `fn without_envs(self: Box<Self>)` implemented for
/// `Command` competes for a `Box<Command>`, and one taking `&self`
/// competes for neither, because the by-value plural is found before
/// probing tries a reference.
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
    let reject = DeepRejectCtxt::relate_rigid_infer(cx.tcx);
    cx.tcx.visible_traits().find(|&other| {
        other != own
            && cx
                .tcx
                .associated_items(other)
                .filter_by_name_unhygienic(name)
                .filter(|item| item.is_method())
                .any(|item| {
                    autoderef_steps(cx, accumulator).any(|step| {
                        may_implement(cx, other, step)
                            && receiver_with_self(cx, item.def_id, step).is_some_and(|receiver| {
                                reject.types_may_unify(accumulator, receiver)
                            })
                    })
                })
    })
}

/// Whether `ty` may implement `other`, for some arguments to the trait's
/// own parameters where it has any.
///
/// Asked of the trait solver with those arguments left to inference, and
/// answered yes where the solver cannot decide: two impls for `ty` with
/// different arguments make the method no less ambiguous. A trait that
/// nothing implements for `ty` competes for nothing.
fn may_implement<'tcx>(cx: &LateContext<'tcx>, other: DefId, ty: Ty<'tcx>) -> bool {
    let tcx = cx.tcx;
    let (infcx, param_env) = tcx.infer_ctxt().build_with_typing_env(cx.typing_env());
    let args = GenericArgs::for_item(tcx, other, |param, _| {
        if param.index == 0 {
            ty.into()
        } else {
            infcx.var_for_def(DUMMY_SP, param)
        }
    });
    let obligation = Obligation::new(
        tcx,
        ObligationCause::dummy(),
        param_env,
        TraitRef::new_from_args(tcx, other, args),
    );
    infcx
        .evaluate_obligation(&obligation)
        .is_ok_and(EvaluationResult::may_apply)
}

/// The receiver type of the trait method `method` with `Self` as
/// `self_ty`, and every other parameter left as a parameter.
fn receiver_with_self<'tcx>(
    cx: &LateContext<'tcx>,
    method: DefId,
    self_ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let args = GenericArgs::for_item(cx.tcx, method, |param, _| {
        if param.index == 0 {
            self_ty.into()
        } else {
            cx.tcx.mk_param_from_def(param)
        }
    });
    cx.tcx
        .fn_sig(method)
        .instantiate(cx.tcx, args)
        .skip_normalization()
        .inputs()
        .skip_binder()
        .first()
        .copied()
}

/// Whether every bound the plural declares is one the rule accounts for.
///
/// Those are `Sized`, `IntoIterator` on what the plural iterates, `AsRef`
/// on what the singular takes, and the pair the `with_env` plural splits,
/// which [`item_fits`] reads. A bound beyond those, `Args: Copy` say, is
/// one the fold does not prove, and nothing then says which iterators the
/// plural accepts, so the answer is no.
pub(super) fn bounds_are_known(cx: &LateContext<'_>, plural: DefId) -> bool {
    let tcx = cx.tcx;
    let item = iterator_item(cx);
    let is = |name: Symbol, def_id: DefId| tcx.is_diagnostic_item(name, def_id);
    // A parameter of the method's own, rather than the trait's `Self`.
    let own_parameter = |ty: Ty<'_>| matches!(ty.kind(), ty::Param(param) if param.index != 0);
    let is_item = |ty: Ty<'_>| item.is_some_and(|item| is_the_item(ty, item));
    tcx.predicates_of(plural)
        .predicates
        .iter()
        .all(|(clause, _)| {
            if let Some(bound) = clause.as_trait_clause() {
                let bound = bound.skip_binder().trait_ref;
                let subject = bound.self_ty();
                return tcx.is_lang_item(bound.def_id, LangItem::Sized)
                    || (is(sym::IntoIterator, bound.def_id) && own_parameter(subject))
                    || (is(sym::Borrow, bound.def_id) && is_item(subject))
                    || (is(sym::AsRef, bound.def_id)
                        && (own_parameter(subject) || is_item(subject)));
            }
            clause.as_projection_clause().is_some_and(|projection| {
                matches!(
                    projection.skip_binder().projection_term.kind,
                    ty::AliasTermKind::ProjectionTy { def_id } if Some(def_id) == item,
                )
            })
        })
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
