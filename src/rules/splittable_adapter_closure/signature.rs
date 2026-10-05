//! What an adapter is, read from the signature it was declared with
//! rather than from a table of names.
//!
//! A table of names is a copy of a dependency's API, and it goes stale
//! the way any copy does: `orx-parallel` renamed its trait from `ParIter`
//! to `Par`, dropped `take_while` and `map_while`, and gained `fold`,
//! across one major release. What did *not* change is the shape each
//! method's closure is declared with, which is what the rule actually
//! needs. `crate::command_extra` makes the same move for a different
//! reason, asking the resolved trait what it declares so that a method
//! added or dropped later needs no version table.
//!
//! The question a trigger asks is which parameter of which argument
//! carries the item, and whether it arrives by value or behind a
//! reference. Both are in the `Fn` bound on the closure the method takes:
//! `Fn(Self::Item) -> Q` hands the item over and `Fn(&Self::Item) -> bool`
//! lends it, and `Fn(&mut B, Self::Item)` says the item is the second
//! parameter because the first is state.

use clippy_utils::sym;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::Symbol;

/// How a method's closure is handed the item.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Handing {
    /// By value, as `Fn(Self::Item) -> Q` does. The item enters and never
    /// comes back out, which is what lets a leading `map` mean the same
    /// thing.
    ByValue,
    /// Behind a reference, as `Fn(&Self::Item) -> bool` does. The item is
    /// handed back downstream, so a leading `map` would change what this
    /// adapter and every later one sees.
    ByReference,
}

/// Where an adapter's item sits and how it arrives.
pub(super) struct Closure {
    /// Which of the method's arguments is the closure carrying the item,
    /// counted past the receiver.
    pub(super) argument: usize,
    /// Which of that closure's parameters is the item. State comes first
    /// where there is any, so a `fold`-shaped closure answers 1.
    pub(super) parameter: usize,
    /// How many parameters the closure takes, which says whether the item
    /// arrives alone.
    pub(super) parameters: usize,
    /// How the item arrives.
    pub(super) handing: Handing,
}

impl Closure {
    /// Whether the closure takes the item and nothing else, which is what
    /// lets the adapter keep the chain's last step.
    pub(super) fn takes_the_item_alone(&self) -> bool {
        self.parameters == 1
    }
}

/// The closure `method` takes that carries `declaring`'s item once, or
/// `None` where it takes no such closure.
///
/// A method may take more than one closure, and one of them may not
/// mention the item at all: `fold` is handed a `Fn() -> B` to make the
/// accumulator alongside the `Fn(&mut B, Self::Item)` that uses it. The
/// one naming the item is the one asked about, which is what tells them
/// apart without naming either.
///
/// A closure handed the item *twice* is no adapter of a single item, and
/// answers `None`. `reduce` is declared `Fn(Self::Item, Self::Item)` and
/// `max_by` is declared `Fn(&Self::Item, &Self::Item)`: both parameters
/// are the item, so a step lifted out of one of them would be applied to
/// the other as well, which is a different program. Reading only the first
/// such parameter would make `reduce` look like `fold`, where the first
/// parameter is an accumulator the lift does not touch.
pub(super) fn closure(cx: &LateContext<'_>, method: DefId, declaring: DefId) -> Option<Closure> {
    let item = associated_item(cx, declaring)?;
    let signature = cx.tcx.fn_sig(method).skip_binder().skip_binder();
    let [_receiver, arguments @ ..] = signature.inputs() else {
        return None;
    };
    cx.tcx
        .predicates_of(method)
        .predicates
        .iter()
        .filter_map(|(clause, _)| clause.as_trait_clause())
        .map(ty::Binder::skip_binder)
        .filter(|bound| cx.tcx.fn_trait_kind_from_def_id(bound.def_id()).is_some())
        .find_map(|bound| {
            // A `Fn` bound carries its parameters as a tuple in the second
            // generic argument, the first being the closure itself.
            let ty::Tuple(parameters) = bound.trait_ref.args.type_at(1).kind() else {
                return None;
            };
            let carried: Vec<_> = parameters
                .iter()
                .enumerate()
                .filter_map(|(index, held)| handing(held, item).map(|how| (index, how)))
                .collect();
            let [(parameter, handing)] = *carried.as_slice() else {
                return None;
            };
            let closure = bound.self_ty();
            let argument = arguments.iter().position(|held| *held == closure)?;
            Some(Closure {
                argument,
                parameter,
                parameters: parameters.len(),
                handing,
            })
        })
}

/// Whether the trait `declaring` declares a method named `name`.
///
/// Asking the trait carries no version table, so a method dropped or
/// added later is covered by the same question. It is what keeps the lift
/// target from being a name the rule merely hopes is there.
pub(super) fn declares(cx: &LateContext<'_>, declaring: DefId, name: &str) -> bool {
    cx.tcx
        .associated_items(declaring)
        .filter_by_name_unhygienic(Symbol::intern(name))
        .any(ty::AssocItem::is_method)
}

/// How `held` carries `item`, or `None` where it is some other type.
fn handing(held: Ty<'_>, item: DefId) -> Option<Handing> {
    match held.kind() {
        ty::Ref(_, referent, _) => is_the_item(*referent, item).then_some(Handing::ByReference),
        _ => is_the_item(held, item).then_some(Handing::ByValue),
    }
}

/// Whether `held` is the associated type `item`.
///
/// It reaches here as a projection rather than as a resolved type, the
/// signature being read as declared rather than as instantiated, which is
/// the point: the question is what the *declaration* says about where the
/// item sits.
fn is_the_item(held: Ty<'_>, item: DefId) -> bool {
    let ty::Alias(_, alias) = held.kind() else {
        return false;
    };
    matches!(alias.kind, ty::AliasTyKind::Projection { def_id } if def_id == item)
}

/// The `Item` associated type `declaring` declares, or `None` for a trait
/// carrying none, which is a trait this reads nothing from.
fn associated_item(cx: &LateContext<'_>, declaring: DefId) -> Option<DefId> {
    cx.tcx
        .associated_items(declaring)
        .filter_by_name_unhygienic(sym::Item)
        .find(|associated| associated.is_type())
        .map(|associated| associated.def_id)
}
