//! `perfectionist::splittable_adapter_predicate` — flag a predicate
//! passed to a filtering adapter that is a conjunction, so one adapter
//! runs every test in it.
//!
//! This is the chain rule's statement reached another way, and it needs
//! a trigger of its own, because the item occurs once per conjunct
//! where the chain rule requires it once in all.
//!
//! What a conjunct lifts into is not `map` but an adapter filtering
//! with the same discipline, which [`crate::adapter_discipline`] is
//! about. Evaluation survives the split untouched: `&&` skips its right
//! operand exactly where the second adapter skips the item, so this
//! trigger needs none of the position care the chain's does.

use crate::adapter_discipline::Discipline;
use crate::binding_uses::names;
use crate::common::{DefaultState, binding_hir_id, binds_mutably, hir_in_external_macro};
use crate::exclusive_captures::exclusive;
use crate::extra_reference::survives;
use crate::receiver_move::movable;
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::paths::{PathNS, lookup_path};
use clippy_utils::{is_from_proc_macro, sym};
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a closure passed to a filtering adapter whose body is a
    /// conjunction of two or more tests, each naming the item, so one
    /// adapter runs all of them.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Both
    /// forms keep the same items, in the same order, running the same
    /// tests the same number of times.
    ///
    /// The preference is that one adapter per test can be read, cut and
    /// instrumented at every test. A test can be deleted by deleting a
    /// line, and the reader sees how many questions are being asked
    /// without parsing an expression to count them.
    ///
    /// ### When it stays silent
    ///
    /// A disjunction does not split: filtering on one of two
    /// alternatives keeps items the pair would have dropped. An adapter
    /// whose answer depends on more than which items satisfy the test
    /// has nothing to lift into, so `all`, `position` and their kin are
    /// left alone: filtering before `all` makes an item that failed the
    /// first test vacuously fine. A conjunct not naming the item is an
    /// invariant to hoist out of the pipeline rather than a test to give
    /// its own adapter.
    ///
    /// A conjunction of comparisons stays whole, because that is how Rust
    /// spells one test: `pos >= range.start && pos < range.end` asks
    /// whether a position is inside a range, and each half on its own
    /// adapter reads worse than the pair does.
    ///
    /// Two more are about what the split would not compile into. `any`
    /// and `Option::is_some_and` hand the item over, where the `filter` a
    /// test lifts into hands a reference, so a test writing to the item
    /// or handing it to something wanting the value is left alone. And
    /// where two tests reach one capture the closure holds other than by
    /// shared borrow, the two closures the split makes cannot both have
    /// it.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let found = paths.filter(|path| path.is_file() && path.exists());
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let found = paths.filter(|path| path.is_file()).filter(|path| path.exists());
    /// ```
    pub perfectionist::SPLITTABLE_ADAPTER_PREDICATE,
    Warn,
    "a predicate passed to a filtering adapter is a conjunction of several tests",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::splittable_adapter_predicate";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct SplittableAdapterPredicate;

impl_lint_pass!(SplittableAdapterPredicate => [SPLITTABLE_ADAPTER_PREDICATE]);

impl Register for rule::SplittableAdapterPredicate {
    /// A conjunction in a predicate is ordinary idiomatic Rust, so the
    /// rule fires often. That is a reason to expect diagnostics rather
    /// than a reason to ship the rule off.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SPLITTABLE_ADAPTER_PREDICATE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(SplittableAdapterPredicate)));
    }
}

/// Which family a receiver belongs to, or `None` where the call is none
/// of this rule's.
///
/// The names are shared across families and with other traits' own
/// methods, so what the call resolves to is what identifies it. rayon's
/// trait carries no diagnostic item, so the path is what identifies that
/// one, and a crate the linted one does not depend on answers nothing.
fn family<'tcx>(
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
        let rayon = [
            Symbol::intern("rayon"),
            Symbol::intern("iter"),
            Symbol::intern("ParallelIterator"),
        ];
        let found = lookup_path(cx.tcx, PathNS::Type, &rayon);
        return found.contains(&declaring).then_some(Family::Rayon);
    }
    let adt = cx
        .typeck_results()
        .expr_ty_adjusted(receiver)
        .peel_refs()
        .ty_adt_def()?;
    cx.tcx
        .is_diagnostic_item(sym::Option, adt.did())
        .then_some(Family::Option)
}

/// Which kind of receiver a filtering adapter is on, which is what
/// tells `Iterator::filter` from `Option::filter`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Iterator,
    Option,
    Rayon,
}

/// The discipline of `method`'s answer on the receiver's kind, or
/// `None` for a method this rule does not speak about.
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

impl<'tcx> LateLintPass<'tcx> for SplittableAdapterPredicate {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(segment, receiver, arguments, _) = expr.kind else {
            return;
        };
        let Some(family) = family(cx, expr, receiver) else {
            return;
        };
        let Some(discipline) = discipline(family, segment.ident.name) else {
            return;
        };
        let Some(closure) = arguments.iter().find_map(|argument| match argument.kind {
            ExprKind::Closure(closure) => Some(closure),
            _ => None,
        }) else {
            return;
        };
        let body = cx.tcx.hir_body(closure.body);
        let [parameter] = body.params else {
            return;
        };
        let Some(item) = binding_hir_id(parameter.pat) else {
            return;
        };
        let item_ty = cx.typeck_results().pat_ty(parameter.pat);
        if hands_the_item_over(segment.ident.name)
            && (item_ty.is_mutable_ptr() || binds_mutably(parameter.pat))
        {
            return;
        }
        if !movable(cx, expr, receiver) {
            return;
        }
        // A conjunction the reader did not write has no `&&` they can
        // cut: the body they see is one macro call.
        if body.value.span.from_expansion() {
            return;
        }
        let mut conjuncts = Vec::new();
        collect(body.value, &mut conjuncts);
        if conjuncts.len() < 2 {
            return;
        }
        // A conjunct not naming the item holds for every item or none,
        // so an adapter of its own would ask the same question once per
        // item. Hoisting it out of the pipeline is the rewrite it
        // wants, which is not the one this rule makes.
        if !conjuncts
            .iter()
            .all(|conjunct| names(cx, conjunct, &[item]))
        {
            return;
        }
        if conjuncts.iter().any(|conjunct| is_comparison(conjunct)) {
            return;
        }
        // Only the lifted tests are asked. The last one stays where it
        // is, with the item it always had.
        let lifted = &conjuncts[..conjuncts.len() - 1];
        if hands_the_item_over(segment.ident.name)
            && !lifted.iter().all(|test| survives(cx, test, item, item_ty))
        {
            return;
        }
        // Each test gets a closure of its own after the split, and two
        // closures cannot both hold a capture held any way but shared, so
        // two tests reaching one does not compile once split.
        let held_alone = exclusive(cx, closure.def_id);
        let reaching = conjuncts
            .iter()
            .filter(|conjunct| names(cx, conjunct, &held_alone))
            .count();
        if reaching > 1 {
            return;
        }
        if segment.ident.span.from_expansion()
            || hir_in_external_macro(cx, expr.hir_id, segment.ident.span)
            || is_from_proc_macro(cx, expr)
        {
            return;
        }
        span_lint_and_then(
            cx,
            SPLITTABLE_ADAPTER_PREDICATE,
            segment.ident.span,
            format!(
                "this predicate runs {} tests on the item, so `{}` does all of them",
                conjuncts.len(),
                segment.ident.name,
            ),
            |diagnostic| {
                diagnostic.help(format!(
                    "lift all but the last test into a leading `{}` of its own, so each \
                     adapter asks one question",
                    discipline.lift_target(),
                ));
            },
        );
    }
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
/// them is how Rust spells one test: `pos >= range.start && pos <
/// range.end` asks whether a position is in a range, and
/// `*byte != b' ' && *byte != b'\t'` whether a byte is whitespace.
/// Giving each half its own adapter reads worse than the conjunction
/// does, so a conjunction holding one is left alone. A conjunct that
/// merely contains a comparison is not one: `wanted(line)` asks a named
/// question however it answers it.
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
