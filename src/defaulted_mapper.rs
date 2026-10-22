//! The call shape the defaulted-mapper rules read.
//!
//! `Option::map_or`, `Option::map_or_else`, `Result::map_or` and
//! `Result::map_or_else` each take a default and a mapper, in that
//! order. `perfectionist::map_or` and `perfectionist::map_or_else`
//! fire on one method name apiece, and what they share is finding the
//! call.
//!
//! What each one does with the call it finds is its own business, but
//! the question of what the default collapses the whole call into is
//! shared, since both rules answer it the same way once the default's
//! value is in hand.

use crate::common::hir_in_external_macro;
use clippy_utils::res::MaybeDef;
use clippy_utils::ty::implements_trait;
use clippy_utils::{is_default_equivalent, is_default_equivalent_call, is_none_expr};
use rustc_ast::LitKind;
use rustc_hir::{Expr, ExprKind, RustcVersion, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty::Ty;
use rustc_span::{Span, sym};

/// The release `Option::map_or_default` and `Result::map_or_default`
/// stabilised in. A fix that calls one does not compile on anything
/// older, so a crate whose `rust-version` predates it is left the help
/// text instead.
pub(crate) const MAP_OR_DEFAULT: RustcVersion = RustcVersion {
    major: 1,
    minor: 99,
    patch: 0,
};

/// Which of the two families the receiver belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Option,
    Result,
}

/// A call to a defaulted mapper, in the parts a diagnostic needs.
pub(crate) struct DefaultedCall<'tcx> {
    pub(crate) family: Family,
    /// The first argument: the default, standing ahead of the mapper it
    /// is a default for.
    pub(crate) default: &'tcx Expr<'tcx>,
    /// The second argument: the mapper, which every remedy keeps.
    pub(crate) mapper: &'tcx Expr<'tcx>,
    /// What the call answers with, which is what a `map_or_default`
    /// would have to have a `Default` for.
    pub(crate) mapped: Ty<'tcx>,
    /// The method name and its arguments, without the receiver or the
    /// dot: `map_or(0, f)` out of `value.map_or(0, f)`. A receiver can
    /// run to several lines and is not what the diagnostic is about.
    pub(crate) span: Span,
}

impl<'tcx> DefaultedCall<'tcx> {
    /// Whether `map_or_default` can answer what this call answers,
    /// given that its default is one the mapped type's `Default`
    /// gives.
    ///
    /// The caller establishes that much, since a value default and a
    /// closure default are written differently. What is checked here
    /// is that the type has a `Default` at all, so the call the fix
    /// writes compiles.
    pub(crate) fn mapped_has_default(&self, cx: &LateContext<'tcx>) -> bool {
        cx.tcx
            .get_diagnostic_item(sym::Default)
            .is_some_and(|default| implements_trait(cx, self.mapped, default, &[]))
    }
}

/// The call `expr` makes to `method` on an `Option` or a `Result`,
/// where the author wrote it themselves.
pub(crate) fn defaulted_call<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    method: &str,
) -> Option<DefaultedCall<'tcx>> {
    let ExprKind::MethodCall(segment, receiver, [default, mapper], span) = expr.kind else {
        return None;
    };
    if segment.ident.name.as_str() != method {
        return None;
    }
    // A call an expansion produced belongs to whoever wrote the macro,
    // and the report would name a line its reader cannot act on, once
    // per expansion site. `report_in_external_macro: false` covers
    // another crate's macro; a `macro_rules!` in the crate being linted
    // needs this. A call written as a macro *argument* keeps the
    // author's own span, so `assert!(opt.map_or(false, f))` is reported.
    if expr.span.from_expansion() {
        return None;
    }
    // The inherent methods and no others. An extension trait can lend
    // the same name to a receiver of the same family, and that call is
    // not this one. A `&Option<T>` whose payload cannot be moved out
    // resolves to the trait's method, since the inherent one takes
    // `self`.
    let method_id = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;
    if cx.tcx.trait_of_assoc(method_id).is_some() {
        return None;
    }
    let family = family(cx, receiver)?;
    // A derive can stamp a user-source span on a call it synthesised,
    // which leaves the `from_expansion` check nothing to find, and the
    // diagnostic span this rule picks is narrower than the call, so
    // `report_in_external_macro: false` does not see the expansion
    // either.
    if hir_in_external_macro(cx, expr.hir_id, span) {
        return None;
    }
    Some(DefaultedCall {
        family,
        default,
        mapper,
        mapped: cx.typeck_results().expr_ty(expr),
        span,
    })
}

/// Which family `receiver`'s type belongs to, if either. The type is
/// taken the way the method call itself reached the receiver: `map_or`
/// takes `self`, so a `&Option<u8>` arrives derefed.
fn family(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<Family> {
    let receiver_type = cx.typeck_results().expr_ty_adjusted(receiver).peel_refs();
    if receiver_type.is_diag_item(cx, sym::Option) {
        return Some(Family::Option);
    }
    receiver_type
        .is_diag_item(cx, sym::Result)
        .then_some(Family::Result)
}

/// Whether `default` is simple enough that meeting it before the
/// mapper costs the reader nothing: a literal, or a path naming a
/// value or a function.
///
/// A closure is none of those, however short its body, since
/// `|| fallback()` spells out a call that the path `fallback` says on
/// its own. Neither is an expression written as a macro call, whatever
/// it expands to, because the call is what the reader meets.
pub(crate) fn default_is_simple(default: &Expr<'_>) -> bool {
    if default.span.from_expansion() {
        return false;
    }
    match default.kind {
        ExprKind::Lit(_) | ExprKind::Path(_) => true,
        // A negated numeric literal is one value to the reader, and an
        // operator over a literal only to the parser.
        ExprKind::Unary(UnOp::Neg, operand) => matches!(operand.kind, ExprKind::Lit(_)),
        _ => false,
    }
}

/// What a default collapses the whole call into, where one call of the
/// receiver's own answers what the two arguments answer together.
pub(crate) enum Collapse {
    /// A predicate of the receiver's own, with the Rust version it
    /// arrived in. That version is younger than the method it
    /// replaces, so the help names the chain beside it.
    Predicate {
        method: &'static str,
        since: &'static str,
    },
    /// `and_then`, which an `Option` has had throughout.
    AndThen,
    /// `map_or_default`, which the rules rewrite the call as.
    Default,
}

/// What `call` collapses into, given `value`: the value its default
/// yields, where the author wrote one. `map_or` always has it; a
/// `map_or_else` has it only where its own `default_value` finds one,
/// and `None` still reaches [`Collapse::Default`].
///
/// The order is what decides the ties. `false` is a `bool`'s `Default`
/// and `None` is an `Option`'s, so both would answer
/// [`Collapse::Default`] as well; the predicate and `and_then` come
/// first because they say what the call tests rather than that its
/// fallback happens to be a default.
pub(crate) fn collapse<'tcx>(
    cx: &LateContext<'tcx>,
    call: &DefaultedCall<'tcx>,
    value: Option<&'tcx Expr<'tcx>>,
) -> Option<Collapse> {
    let predicate = |method, since| Some(Collapse::Predicate { method, since });
    if let Some(value) = value {
        if let ExprKind::Lit(literal) = value.kind
            && let LitKind::Bool(flag) = literal.node
        {
            return match (call.family, flag) {
                (Family::Option, false) => predicate("is_some_and", "1.70"),
                (Family::Option, true) => predicate("is_none_or", "1.82"),
                (Family::Result, false) => predicate("is_ok_and", "1.70"),
                // A `Result` has no counterpart of `is_none_or`.
                (Family::Result, true) => None,
            };
        }
        if call.family == Family::Option && is_none_expr(cx, value) {
            return Some(Collapse::AndThen);
        }
    }
    // Which values count as a default is `is_default_equivalent`'s
    // judgement. A `Foo::new` of the crate's own counts only where the
    // default is written as a value: comparing `Foo::default`'s body
    // against the call needs the call, which the path form is not
    // handed.
    let yields_default = match value {
        Some(value) => is_default_equivalent(cx, value),
        None => is_default_equivalent_call(cx, call.default, None),
    };
    (yields_default && call.mapped_has_default(cx)).then_some(Collapse::Default)
}
