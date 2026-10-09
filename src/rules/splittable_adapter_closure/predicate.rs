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
use super::signature::{self, Handing};
use super::{Call, Finding, Fix, only_closure};
use crate::adapter_discipline::Discipline;
use crate::binding_uses::names;
use crate::common::{binding_hir_id, binds_mutably, drops_a_comment};
use crate::exclusive_captures::exclusive;
use crate::extra_reference::survives;
use clippy_utils::SpanlessEq;
use clippy_utils::source::snippet_with_applicability;
use rustc_errors::Applicability;
use rustc_hir::{self as hir, BinOpKind, CaptureBy, Closure, Expr, ExprKind, Pat, TyKind};
use rustc_lint::LateContext;

/// The conjunction this adapter's predicate is, where it runs two or more
/// tests on the item.
pub(super) fn check<'tcx>(cx: &LateContext<'tcx>, call: &Call<'tcx>) -> Option<Finding> {
    let method = call.method;
    let discipline = discipline(cx, call)?;
    let closure = only_closure(call.arguments)?;
    let body = cx.tcx.hir_body(closure.body);
    let [parameter] = body.params else {
        return None;
    };
    let item = binding_hir_id(parameter.pat)?;
    let item_ty = cx.typeck_results().pat_ty(parameter.pat);
    if hands_the_item_over(cx, call) && (item_ty.is_mutable_ptr() || binds_mutably(parameter.pat)) {
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
    if bounds_one_quantity(cx, &conjuncts) {
        return None;
    }
    // Only the lifted tests are asked. The last one stays where it is,
    // with the item it always had.
    let lifted = &conjuncts[..conjuncts.len() - 1];
    if hands_the_item_over(cx, call) && !lifted.iter().all(|test| survives(cx, test, item, item_ty))
    {
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
        moves_the_receiver: true,
        fix: fix(cx, call, closure, parameter.pat, &conjuncts, discipline),
    })
}

/// The rewrite: one adapter per test, in the order the conjunction asked
/// them.
///
/// Each test keeps its own text and gets a closure naming the item the way
/// the folded one did, which is always in scope, each closure having a
/// scope of its own. The closures are not reduced to paths:
/// `clippy::redundant_closure_for_method_calls` is the lint that reduces
/// the ones that can be reduced, and it knows which those are.
///
/// `None` where the text cannot be assembled from the parts: a parameter
/// whose type is written out names a type the lifted adapter hands one
/// reference deeper, and a pattern other than a plain binding would have to
/// be reproduced per closure.
fn fix<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Call<'tcx>,
    closure: &'tcx Closure<'tcx>,
    parameter: &'tcx Pat<'tcx>,
    conjuncts: &[&'tcx Expr<'tcx>],
    discipline: Discipline,
) -> Option<Fix> {
    if !matches!(
        closure.fn_decl.inputs,
        [hir::Ty {
            kind: TyKind::Infer(()),
            ..
        },],
    ) {
        return None;
    }
    let (last, lifted) = conjuncts.split_last()?;
    let mut applicability = match drops_a_comment(
        cx,
        call.fix_span,
        conjuncts.iter().map(|conjunct| conjunct.span),
    ) {
        true => Applicability::Unspecified,
        false => Applicability::MachineApplicable,
    };
    // A `move` closure's captures are each held by a copy or a shared
    // borrow, which the gates above answered for, so every closure the
    // split writes keeps the keyword.
    let moves = match closure.capture_clause {
        CaptureBy::Value { .. } => "move ",
        _ => "",
    };
    let item = snippet_with_applicability(cx, parameter.span, "..", &mut applicability);
    let mut test = |conjunct: &Expr<'tcx>, adapter: &str| -> String {
        format!(
            "{adapter}({moves}|{item}| {})",
            snippet_with_applicability(cx, conjunct.span, "..", &mut applicability),
        )
    };
    let mut suggestion = String::new();
    for conjunct in lifted {
        suggestion.push_str(&test(conjunct, discipline.lift_target()));
        suggestion.push('.');
    }
    suggestion.push_str(&test(last, &method_name(call)));
    Some(Fix {
        span: call.fix_span,
        suggestion,
        applicability,
        // This rewrite writes each test as the closure the reader had, so it
        // names no path that could be missing.
        imports: Vec::new(),
        inject_use: cx.tcx.hir_root_module().spans.inject_use_span,
    })
}

/// The adapter's name as the reader wrote it, which the last test keeps.
fn method_name(call: &Call<'_>) -> String {
    call.method.to_string()
}

/// The discipline of `method`'s answer on `family`, or `None` for a
/// method this trigger does not speak about.
fn discipline(cx: &LateContext<'_>, call: &Call<'_>) -> Option<Discipline> {
    let (family, method) = (call.receiver.family, call.method);
    if family == Family::Orx {
        return derived(cx, call);
    }
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

/// The discipline `call` answers with, read from the signature it was
/// declared with where the family is not tabled.
///
/// A filtering adapter is one handed the item behind a reference and
/// answering `bool`, which is what `filter` and its kin are and what
/// separates them from the mapping adapters the chain trigger reads. How
/// many parameters it takes is not asked here, because [`check`] reads the
/// closure's only parameter and turns away a closure with any other number.
///
/// Two facts a signature cannot carry stay by name. A prefix-shaped
/// adapter is declared exactly as a set-shaped one, `Fn(&Item) -> bool`
/// either way, so which of them stops the run is the name's to say. And
/// `all` is excluded because filtering before it makes an item that failed
/// the first test vacuously fine, which is a property of the answer rather
/// than of the signature.
///
/// The trait is not asked whether it declares the prefix-shaped adapter,
/// because the name having matched is already that answer: a call resolved
/// to `take_while` is a `take_while` the trait declares.
fn derived(cx: &LateContext<'_>, call: &Call<'_>) -> Option<Discipline> {
    let declaring = call.receiver.declaring?;
    if call.method.as_str() == "all" {
        return None;
    }
    let closure = signature::closure(cx, call.receiver.method, declaring)?;
    if closure.handing != Handing::ByReference {
        return None;
    }
    Some(match call.method.as_str() == PREFIX_SHAPED {
        true => Discipline::Prefix,
        false => Discipline::Set,
    })
}

/// The adapter that stops a run rather than sieving it, where the family
/// declares one.
const PREFIX_SHAPED: &str = "take_while";

/// Whether `method` hands the item to its closure by value, where the
/// lift target hands it by reference.
///
/// `filter` and `take_while` take `&Item`, so a test lifted out of an
/// adapter that was handed the item itself gets one reference more than
/// it had, and [`crate::extra_reference`] is about what survives that.
fn hands_the_item_over(cx: &LateContext<'_>, call: &Call<'_>) -> bool {
    // Where the signature is what the family is read from, it answers this
    // too: `orx-parallel`'s `any` lends the item where `Iterator`'s hands it
    // over, and the name is the same either way.
    if let Some(declaring) = call.receiver.declaring
        && call.receiver.family == Family::Orx
    {
        return signature::closure(cx, call.receiver.method, declaring)
            .is_some_and(|closure| closure.handing == Handing::ByValue);
    }
    matches!(call.method.as_str(), "any" | "is_some_and")
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

/// Whether `conjuncts` are comparisons that all bound one quantity, which
/// is how Rust spells a single test.
///
/// `pos >= range.start && pos < range.end` asks whether a position is in
/// a range, and `*byte != b' ' && *byte != b'\t'` whether a byte is
/// whitespace. Each reads as one question because every comparison bounds
/// the same value, so giving the halves an adapter apiece reads worse than
/// the conjunction does.
///
/// Comparing is not enough on its own. `row.first >= low && row.second < high`
/// bounds two quantities, so it asks two questions however it is spelled,
/// and an adapter apiece is what it wants. [`shared_operand`] is the
/// difference.
///
/// One comparison among named questions is a third shape:
/// `wanted(line) && line.len() > 3` asks two things already, and the
/// bound is one of them rather than half of one. A conjunct that merely
/// contains a comparison is not a comparison either: `wanted(line)` asks
/// a named question however it answers it.
fn bounds_one_quantity<'tcx>(cx: &LateContext<'tcx>, conjuncts: &[&'tcx Expr<'tcx>]) -> bool {
    let Some(operands) = conjuncts
        .iter()
        .map(|conjunct| comparison_operands(conjunct))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    shared_operand(cx, &operands)
}

/// The two sides of `expr` where it compares them, `None` otherwise.
fn comparison_operands<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<[&'tcx Expr<'tcx>; 2]> {
    let ExprKind::Binary(operator, left, right) = expr.kind else {
        return None;
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
    .then_some([left, right])
}

/// Whether one quantity is an operand of every comparison in `operands`.
///
/// Which side it falls on is not asked, so `range.start <= pos && pos <
/// range.end` reads as the one range check it is.
///
/// Side effects are not denied, so two readings of `line.len()` count as
/// one quantity. `line.len() > 3 && line.len() < 80` is the length range
/// the exemption is for, and denying them would split it. The cost is that
/// a quantity read twice through a mutating call, `counter.bump() > 3 &&
/// counter.bump() < 80`, reads as one where it is two; that direction
/// leaves a predicate folded rather than asking for a split the reader
/// should not take.
fn shared_operand<'tcx>(cx: &LateContext<'tcx>, operands: &[[&'tcx Expr<'tcx>; 2]]) -> bool {
    let Some((first, rest)) = operands.split_first() else {
        return false;
    };
    let mut equal = SpanlessEq::new(cx);
    first.iter().any(|quantity| {
        rest.iter().all(|pair| {
            pair.iter()
                .any(|other| equal.eq_expr(quantity.span.ctxt(), quantity, other))
        })
    })
}
