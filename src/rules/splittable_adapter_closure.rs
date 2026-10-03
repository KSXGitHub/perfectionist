//! `perfectionist::splittable_adapter_closure` — flag a closure passed
//! to an iterator adapter that chains two or more steps onto its item,
//! so one adapter does all of them.
//!
//! The trigger walks *up* from the item parameter's occurrence, because
//! what makes a parent a step is that the chain so far is what it is
//! applied to; walking down would have to guess which sub-expression the
//! chain runs through. [`chain`] does that walk, [`adapter`] says which
//! methods are in scope and where their item sits, and [`position`]
//! answers the one question whose wrong answer compiles.
//!
//! Three gates stand between the walk and a diagnostic, and they fail
//! differently. The item occurring more than once leaves a split with a
//! binding to name that no longer exists, so the suggestion would be
//! `E0425`. A step whose result borrows the item would be `E0515` once
//! lifted. A chain in a conditionally-evaluated position, though, splits
//! into something that compiles and behaves differently, which is why
//! [`position`] is where the care goes.
//!
//! Scope: `Iterator` and `DoubleEndedIterator`, and steps taking their
//! receiver by value. The
//! planning file's other families and its other two triggers are not
//! implemented; `planned-rules/splittable-adapter-closure.md` records
//! which.

use crate::binding_uses::{names, uses};
use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::{is_from_proc_macro, sym};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::{BorrowKind, CapturedPlace, Ty, UpvarCapture};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod adapter;
mod chain;
mod position;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a closure passed to an iterator adapter where the chain of
    /// steps rooted at the closure's item is two or more long, so one
    /// adapter performs all of them.
    ///
    /// A chain starts at the item and runs outwards: a method call whose
    /// receiver is the chain so far, or a call whose sole argument is
    /// it, naming no other parameter of the closure.
    ///
    /// The adapters in scope are the ones whose item enters the closure
    /// by value and never comes back out, which is what lets a leading
    /// `map` mean the same thing. `filter`, `find`, `take_while`,
    /// `inspect` and their kin are left alone, because a `map` in front
    /// of them changes what they see.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Both
    /// forms compute the same answer, with the same laziness and the
    /// same short-circuiting, and a closure that chains two calls is
    /// ordinary idiomatic Rust.
    ///
    /// The preference is that a pipeline written one stage per adapter
    /// can be read, cut and instrumented at every stage. An `inspect`
    /// can go between any two of them, a stage can be deleted by
    /// deleting a line, and the reader sees the same number of steps the
    /// data goes through. Folded into a closure, those same steps are an
    /// expression to be parsed before any of them can be found.
    ///
    /// ### When it stays silent
    ///
    /// A chain of one step has nothing to split. An item named more than
    /// once cannot be split at all, since each step would get its own
    /// closure and the later mentions would have no binding to name. A
    /// step whose result borrows the item cannot be lifted out of the
    /// closure the item belongs to. And a chain the closure does not
    /// always reach is left alone, because lifting it would run it for
    /// every item rather than for some.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let names = headers
    ///     .map(|header| header.trim().to_ascii_lowercase())
    ///     .collect::<Vec<_>>();
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let names = headers
    ///     .map(str::trim)
    ///     .map(str::to_ascii_lowercase)
    ///     .collect::<Vec<_>>();
    /// ```
    pub perfectionist::SPLITTABLE_ADAPTER_CLOSURE,
    Warn,
    "a closure passed to an iterator adapter chains several steps onto its item",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::splittable_adapter_closure";

/// What a reader does with the diagnostic, which names the count rather
/// than rendering the split: the point-free form a split invites asks
/// more of each step than the split does, so the text a rewrite would
/// have to choose is not the text a reader wants.
const SPLIT_HELP: &str = "lift each step into a leading `map` of its own, leaving the closure \
                          only the work that is the adapter's";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct SplittableAdapterClosure;

impl_lint_pass!(SplittableAdapterClosure => [SPLITTABLE_ADAPTER_CLOSURE]);

impl Register for rule::SplittableAdapterClosure {
    /// The shape the rule flags is ordinary idiomatic Rust, so it fires
    /// often. That is a reason to expect diagnostics rather than a
    /// reason to ship the rule off: firing on what the rule names is not
    /// a false positive.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SPLITTABLE_ADAPTER_CLOSURE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(SplittableAdapterClosure)));
    }
}

impl<'tcx> LateLintPass<'tcx> for SplittableAdapterClosure {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(segment, _, arguments, _) = expr.kind else {
            return;
        };
        let Some(shape) = adapter::shape(segment.ident.name) else {
            return;
        };
        // The names are shared with inherent methods and with other
        // traits' own adapters, whose items need not enter by value, so
        // the trait the call resolves to is what identifies it.
        let Some(method) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
            return;
        };
        let Some(declaring) = cx.tcx.trait_of_assoc(method) else {
            return;
        };
        // `rfold` and `try_rfold` are `DoubleEndedIterator`'s, the same
        // shape worked from the other end. They lift into
        // `Iterator::map` all the same, because `Map` is double-ended
        // wherever its iterator is.
        let declares = |iterator| cx.tcx.is_diagnostic_item(iterator, declaring);
        if !declares(sym::Iterator) && !declares(sym::DoubleEndedIterator) {
            return;
        }
        let Some(closure) = arguments.iter().find_map(|argument| match argument.kind {
            ExprKind::Closure(closure) => Some(closure),
            _ => None,
        }) else {
            return;
        };
        let body = cx.tcx.hir_body(closure.body);
        let Some(parameter) = body.params.get(shape.item_parameter()) else {
            return;
        };
        let Some(item) = chain::binding(parameter.pat) else {
            return;
        };
        // Each step gets its own closure after the split, so a second
        // use of the item would be left with nothing to name.
        let occurrences = uses(cx, body.value, &[item]);
        let [root] = *occurrences.as_slice() else {
            return;
        };
        let parameters: Vec<_> = body
            .params
            .iter()
            .filter_map(|parameter| chain::binding(parameter.pat))
            .collect();
        let steps = chain::steps(cx, root, &parameters);
        if steps.len() < 2 {
            return;
        }
        // The last step stays with the adapter where the closure is
        // unary, so only the ones moving into a `map` have to be
        // liftable.
        let lifted = match shape.keeps_the_last_step() {
            true => &steps[..steps.len() - 1],
            false => &steps[..],
        };
        if !lifted.iter().all(|step| is_liftable(cx, step.expr)) {
            return;
        }
        // A lifted step runs in a closure of its own, alongside the one
        // the adapter keeps. Two closures cannot both hold a mutable
        // borrow of the same capture, so a step reaching one is `E0499`
        // once lifted.
        let mutably_captured = mutable_captures(cx, closure.def_id);
        if lifted
            .iter()
            .any(|step| names(cx, step.expr, &mutably_captured))
        {
            return;
        }
        let top = steps.last().expect("two or more steps").expr;
        let anchored = match shape.keeps_the_last_step() {
            // Requiring the chain to *be* the body satisfies the
            // position condition vacuously, and holds the scope to a
            // closure whose whole job is the chain.
            true => top.hir_id == body.value.hir_id,
            false => {
                position::always_evaluated(cx, top.hir_id, body.value.hir_id)
                    && position::nothing_diverts_first(body, root.span)
            }
        };
        if !anchored {
            return;
        }
        // The diagnostic span is the adapter's method segment, which a
        // derive can stamp with a user-source span, defeating both
        // `report_in_external_macro: false` and `hir_in_external_macro`.
        if segment.ident.span.from_expansion()
            || hir_in_external_macro(cx, expr.hir_id, segment.ident.span)
            || is_from_proc_macro(cx, expr)
        {
            return;
        }
        span_lint_and_then(
            cx,
            SPLITTABLE_ADAPTER_CLOSURE,
            segment.ident.span,
            format!(
                "this closure chains {} steps onto the item, so `{}` does all of them",
                steps.len(),
                segment.ident.name,
            ),
            |diagnostic| {
                diagnostic.help(SPLIT_HELP);
            },
        );
    }
}

/// Whether lifting `step` out of the closure keeps it compiling.
///
/// A lifted step runs in a `map` of its own, applied to a local that
/// dies at the end of it, so a result borrowing that local is `E0515`.
/// Two shapes rule that out. A result carrying no lifetime borrows
/// nothing. And where what the step is applied to is itself a
/// reference, a `&self` borrows the referent rather than the local
/// pointer, and the referent outlives the closure:
/// `.map(|s: &str| s.trim())` hands back a borrow of what `s` points
/// at.
///
/// The question is about the step's own receiver, which is the item
/// only for the first step. Each later one is applied to the step
/// before it, so `.map(|s: &str| s.to_lowercase().trim())` has an
/// owned `String` under its `trim` however the item arrived.
///
/// Regions are erased by the time typeck results are read, so a
/// result's lifetime cannot be matched against the receiver's. These
/// two clauses are the conservative answer that needs no such match.
fn is_liftable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    if !borrows(cx.typeck_results().expr_ty(step)) {
        return true;
    }
    let Some(receiver) = applied_to(step) else {
        return false;
    };
    cx.typeck_results().expr_ty(receiver).is_ref()
}

/// What `step` applies itself to: a method call's receiver, or the sole
/// argument of a call.
fn applied_to<'tcx>(step: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    match step.kind {
        ExprKind::MethodCall(_, receiver, ..) => Some(receiver),
        ExprKind::Call(_, [only]) => Some(only),
        _ => None,
    }
}

/// The locals this closure captures in a way a second closure could
/// not also hold: a mutable borrow, a unique immutable borrow, or a
/// move. A shared borrow is left out, since any number of closures may
/// hold one.
fn mutable_captures<'tcx>(cx: &LateContext<'tcx>, closure: LocalDefId) -> Vec<HirId> {
    cx.typeck_results()
        .closure_min_captures_flattened(closure)
        .filter(|capture| {
            !matches!(
                capture.info.capture_kind,
                UpvarCapture::ByRef(BorrowKind::Immutable),
            )
        })
        .map(CapturedPlace::get_root_variable)
        .collect()
}

/// Whether `ty` mentions a region, which is where a borrow would show
/// up.
fn borrows(ty: Ty<'_>) -> bool {
    ty.walk().any(|argument| argument.as_region().is_some())
}
