//! The conjunction trigger: a predicate passed to a filtering adapter
//! that is a conjunction, so one adapter runs every test in it.
//!
//! This is the chain trigger's statement reached another way, and it
//! needs a trigger of its own, because the item occurs once per conjunct
//! where the chain trigger requires it once in all.
//!
//! What a conjunct lifts into is not `map` but an adapter filtering with
//! the same discipline, which [`crate::adapter_discipline`] is about.
//! Evaluation survives the split untouched: `&&` skips its right operand
//! exactly where the second adapter skips the item, so this trigger needs
//! none of the position care the chain's does.

use super::family::Family;
use super::{Finding, only_closure};
use crate::adapter_discipline::Discipline;
use crate::binding_uses::names;
use crate::common::{binding_hir_id, binds_mutably};
use crate::exclusive_captures::exclusive;
use crate::extra_reference::survives;
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// The conjunction this adapter's predicate is, where it runs two or more
/// tests on the item.
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    method: Symbol,
    arguments: &'tcx [Expr<'tcx>],
    family: Family,
) -> Option<Finding> {
    let discipline = discipline(family, method)?;
    let closure = only_closure(arguments)?;
    let body = cx.tcx.hir_body(closure.body);
    let [parameter] = body.params else {
        return None;
    };
    let item = binding_hir_id(parameter.pat)?;
    let item_ty = cx.typeck_results().pat_ty(parameter.pat);
    if hands_the_item_over(method) && (item_ty.is_mutable_ptr() || binds_mutably(parameter.pat)) {
        return None;
    }
    // A conjunction the reader did not write has no `&&` they can cut:
    // the body they see is one macro call.
    if body.value.span.from_expansion() {
        return None;
    }
    let mut conjuncts = Vec::new();
    collect(body.value, &mut conjuncts);
    if conjuncts.len() < 2 {
        return None;
    }
    // A conjunct not naming the item holds for every item or none, so an
    // adapter of its own would ask the same question once per item.
    // Hoisting it out of the pipeline is the rewrite it wants, which is
    // not the one this trigger makes.
    if !conjuncts
        .iter()
        .all(|conjunct| names(cx, conjunct, &[item]))
    {
        return None;
    }
    if conjuncts.iter().all(|conjunct| is_comparison(conjunct)) {
        return None;
    }
    // Only the lifted tests are asked. The last one stays where it is,
    // with the item it always had.
    let lifted = &conjuncts[..conjuncts.len() - 1];
    if hands_the_item_over(method) && !lifted.iter().all(|test| survives(cx, test, item, item_ty)) {
        return None;
    }
    // Each test gets a closure of its own after the split, and a capture
    // only a shared borrow or a copy of a `Copy` value lets both of them
    // hold, so two tests reaching any other one does not compile once
    // split.
    let held_alone = exclusive(cx, closure.def_id);
    let reaching = conjuncts
        .iter()
        .filter(|conjunct| names(cx, conjunct, &held_alone))
        .count();
    if reaching > 1 {
        return None;
    }
    Some(Finding {
        message: format!(
            "this predicate runs {} tests on the item, so `{method}` does all of them",
            conjuncts.len(),
        ),
        help: format!(
            "give every test but the last a leading `{}` of its own, so each \
             adapter asks one question",
            discipline.lift_target(),
        ),
    })
}

/// The discipline of `method`'s answer on `family`, or `None` for a
/// method this trigger does not speak about.
fn discipline(family: Family, method: Symbol) -> Option<Discipline> {
    // `all`, `position`, `rposition`, `partition`, `skip_while` and
    // `Option::is_none_or` take a predicate of the same shape and have no
    // lift target: filtering first flips what `all` and `is_none_or`
    // answer, renumbers `position`'s, and loses what the other two were
    // counting on. `Result` has no filtering adapter at all, so
    // `is_ok_and` and `is_err_and` stay folded however they are written.
    Some(match (family, method.as_str()) {
        (Family::Iterator, "filter" | "find" | "rfind" | "any") => Discipline::Set,
        (Family::Iterator, "take_while") => Discipline::Prefix,
        (Family::Option, "filter" | "is_some_and") => Discipline::Set,
        (Family::Rayon, "filter" | "find_first" | "find_any" | "any") => Discipline::Set,
        _ => return None,
    })
}

/// Whether `method` hands the item to its closure by value, where the
/// lift target hands it by reference.
///
/// `filter` and `take_while` take `&Item`, so a test lifted out of an
/// adapter that was handed the item itself gets one reference more than
/// it had, and [`crate::extra_reference`] is about what survives that.
fn hands_the_item_over(method: Symbol) -> bool {
    matches!(method.as_str(), "any" | "is_some_and")
}

/// Flattens `a && b && c` into its conjuncts, outermost last.
///
/// A conjunct that is itself a disjunction stays whole: `||` is not a
/// conjunction, and an adapter of its own is what it would get.
fn collect<'tcx>(expr: &'tcx Expr<'tcx>, conjuncts: &mut Vec<&'tcx Expr<'tcx>>) {
    if let ExprKind::Binary(operator, left, right) = expr.kind
        && operator.node == BinOpKind::And
    {
        collect(left, conjuncts);
        collect(right, conjuncts);
        return;
    }
    conjuncts.push(expr);
}

/// Whether `expr` compares two values.
///
/// A comparison is a bound rather than a question, and a conjunction of
/// nothing but comparisons is how Rust spells one test:
/// `pos >= range.start && pos < range.end` asks whether a position is in
/// a range, and `*byte != b' ' && *byte != b'\t'` whether a byte is
/// whitespace. Giving each half its own adapter reads worse than the
/// conjunction does, so a conjunction of them is left alone.
///
/// One comparison among named questions is a different shape:
/// `wanted(line) && line.len() > 3` asks two things already, and the
/// bound is one of them rather than half of one. A conjunct that merely
/// contains a comparison is not a comparison either: `wanted(line)` asks
/// a named question however it answers it.
fn is_comparison(expr: &Expr<'_>) -> bool {
    let ExprKind::Binary(operator, ..) = expr.kind else {
        return false;
    };
    matches!(
        operator.node,
        BinOpKind::Eq
            | BinOpKind::Ne
            | BinOpKind::Lt
            | BinOpKind::Le
            | BinOpKind::Gt
            | BinOpKind::Ge,
    )
}
