//! The singular/plural pairs this rule is about, and the trait that
//! declares them.
//!
//! The set is fixed rather than configured. The suggestion is sound
//! only because upstream defines each plural as exactly the fold of its
//! singular -- `with_args` is `args.into_iter().fold(self,
//! Self::with_arg)` -- so a pair named for some other builder could not
//! promise the same.

use clippy_utils::ty::get_iterator_item_ty;
use rustc_hir::Expr;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, AssocItem};
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

/// Whether the fold's item is the two-element tuple a splitting plural
/// destructures.
///
/// `with_envs` takes `IntoIterator<Item = (Key, Value)>`, and a
/// reference to a pair is not a pair: `&[(&str, &str)]` yields
/// `&(&str, &str)`, which the *fold* takes -- `|command, (key, value)|`
/// binds through the reference -- and the plural does not. Asking the
/// item rather than the pattern is what tells the two apart, since the
/// pattern is identical either way.
pub(super) fn item_splits(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    let receiver_ty = cx.typeck_results().expr_ty(receiver);
    get_iterator_item_ty(cx, receiver_ty)
        .is_some_and(|item| matches!(item.kind(), ty::Tuple(elements) if elements.len() == 2))
}

/// Whether `trait_id` is `command_extra::CommandExtra`.
///
/// The name alone would match a `CommandExtra` of the author's own, so
/// the crate is asked for too.
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
pub(super) fn declares(cx: &LateContext<'_>, trait_id: DefId, plural: &str) -> bool {
    cx.tcx
        .associated_items(trait_id)
        .filter_by_name_unhygienic(Symbol::intern(plural))
        .any(AssocItem::is_method)
}
