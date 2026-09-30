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
use rustc_middle::ty::{self, AssocItem, Ty};
use rustc_span::Symbol;

/// The crate `command-extra` compiles under, as the compiler spells it
/// rather than as Cargo does.
const CRATE: &str = "command_extra";

/// The trait whose setters come in pairs.
const TRAIT: &str = "CommandExtra";

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

/// Whether the fold's item is a shape the plural's own bound accepts.
///
/// A plural that splits an item constrains what it will take, and which
/// constraint that is depends on the release. 1.1.0 and 1.2.0 write
/// `Envs: IntoIterator<Item = (Key, Value)>`, an associated-type
/// equality that no reference satisfies: `&[(&str, &str)]` yields
/// `&(&str, &str)`, which the *fold* takes -- `|command, (key, value)|`
/// binds through the reference -- and the plural does not. 1.3.0 writes
/// `Envs::Item: Borrow<(Key, Value)>`, which a reference to a pair does
/// satisfy.
///
/// Reading the constraint keeps this in step with the version resolved,
/// as [`declares`] does for the plural's existence. Asking the item rather
/// than the closure's pattern is what tells the shapes apart at all,
/// since the pattern is identical either way.
pub(super) fn item_fits<'tcx>(cx: &LateContext<'tcx>, plural: DefId, receiver: &Expr<'_>) -> bool {
    let receiver_ty = cx.typeck_results().expr_ty(receiver);
    let Some(item) = get_iterator_item_ty(cx, receiver_ty) else {
        return false;
    };
    let Some(assoc_item) = iterator_item(cx) else {
        return true;
    };
    for (clause, _) in cx.tcx.predicates_of(plural).predicates {
        // `Item = (Key, Value)`, so the item has to be that tuple.
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
        // `Item: SomeTrait<(Key, Value)>`, so whatever that trait
        // accepts. The solver answers, rather than a rule of this
        // module's own about how many references the trait's impls see
        // through.
        if let Some(bound) = clause.as_trait_clause() {
            let bound = bound.skip_binder().trait_ref;
            if is_the_item(bound.self_ty(), assoc_item)
                && let Some(required) = bound.args.types().nth(1)
                && let ty::Tuple(required) = required.kind()
            {
                return satisfies(cx, item, bound.def_id, required.len());
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

/// Whether `item` satisfies `bound` for a tuple of `arity` elements.
///
/// The tuple the trait is asked about is `item` with its references
/// stripped, which is the only candidate worth trying: the plural
/// destructures a tuple, so nothing else could be split at all.
fn satisfies<'tcx>(cx: &LateContext<'tcx>, item: Ty<'tcx>, bound: DefId, arity: usize) -> bool {
    let candidate = item.peel_refs();
    matches!(candidate.kind(), ty::Tuple(elements) if elements.len() == arity)
        && implements_trait(cx, item, bound, &[candidate.into()])
}

pub(super) fn is_command_extra(cx: &LateContext<'_>, trait_id: DefId) -> bool {
    cx.tcx.item_name(trait_id) == Symbol::intern(TRAIT)
        && cx.tcx.crate_name(trait_id.krate) == Symbol::intern(CRATE)
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
