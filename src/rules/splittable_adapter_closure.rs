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
//! Scope: steps taking their receiver by value, over the families
//! [`adapter::Family`] names. The planning file's remaining families are
//! not implemented, and its other two triggers are rules of their own;
//! `planned-rules/splittable-adapter-closure.md` records both.

use self::adapter::{Adapter, Family};
use crate::binding_uses::{names, uses};
use crate::common::{DefaultState, hir_in_external_macro};
use crate::exclusive_captures::exclusive;
use crate::receiver_move::movable;
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::paths::{PathNS, lookup_path};
use clippy_utils::ty::implements_trait;
use clippy_utils::{is_from_proc_macro, sym};
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::Ty;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

mod adapter;
mod chain;
mod position;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a closure passed to a mapping adapter where the chain of
    /// steps rooted at the closure's item is two or more long, so one
    /// adapter performs all of them. The receivers in scope are
    /// iterators, `Option`, `Result`, `Poll`, `ControlFlow`, and the
    /// `Itertools`, `ParallelIterator` and `Pipe` traits.
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
    /// step whose result borrows lifts only where it borrows through a
    /// reference it was handed, so one borrowing an owned item, or a
    /// temporary the closure made, stays. And a chain the closure does not
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
    "a closure passed to a mapping adapter chains several steps onto its item",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::splittable_adapter_closure";

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
        let ExprKind::MethodCall(segment, receiver, arguments, _) = expr.kind else {
            return;
        };
        let Some(family) = family(cx, expr, receiver) else {
            return;
        };
        let Some(adapter) = adapter::adapter(family, segment.ident.name) else {
            return;
        };
        check(cx, expr, segment, receiver, arguments, family, adapter);
    }
}

/// Which family `receiver` belongs to, or `None` where the call is none
/// of this rule's.
///
/// The method names are shared across families and with other traits'
/// own adapters, whose items need not enter by value, so what the call
/// resolves to is what identifies it. `rfold` and `try_rfold` are
/// `DoubleEndedIterator`'s, the same shape worked from the other end;
/// they lift into `Iterator::map` all the same, because `Map` is
/// double-ended wherever its iterator is.
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
        // A trait from a crate the linted one does not depend on resolves
        // to nothing, so these cost a lookup and answer `None` there.
        return extension(cx, declaring);
    }
    let adt = cx
        .typeck_results()
        .expr_ty_adjusted(receiver)
        .peel_refs()
        .ty_adt_def()?;
    let carries = |name| cx.tcx.is_diagnostic_item(name, adt.did());
    let named = [
        (sym::Option, Family::Option),
        (sym::Result, Family::Result),
        (sym::ControlFlow, Family::ControlFlow),
    ]
    .into_iter()
    .find_map(|(name, family)| carries(name).then_some(family));
    // `Poll` carries no diagnostic item, measured by asking
    // `get_diagnostic_name` for it, so the path is what identifies it.
    named.or_else(|| {
        let poll = lookup_path(
            cx.tcx,
            PathNS::Type,
            &[sym::core, Symbol::intern("task"), sym::Poll],
        );
        poll.contains(&adt.did()).then_some(Family::Poll)
    })
}

/// Which extension trait `declaring` is, or `None` for a trait this rule
/// does not speak about.
///
/// None of these carries a diagnostic item, so the path is what
/// identifies it. `Itertools` is a blanket extension of `Iterator` and
/// `Pipe` one of everything, which is why a receiver's type says nothing
/// about either.
fn extension(cx: &LateContext<'_>, declaring: DefId) -> Option<Family> {
    let intern = Symbol::intern;
    let paths = [
        (
            vec![intern("itertools"), intern("Itertools")],
            Family::Itertools,
        ),
        (
            vec![intern("rayon"), intern("iter"), intern("ParallelIterator")],
            Family::Rayon,
        ),
        (vec![intern("pipe_trait"), intern("Pipe")], Family::Pipe),
    ];
    paths.into_iter().find_map(|(path, family)| {
        let found = lookup_path(cx.tcx, PathNS::Type, &path);
        found.contains(&declaring).then_some(family)
    })
}

/// Flag the closure this adapter holds where the chain rooted at its
/// item is two or more steps long.
fn check<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    segment: &'tcx rustc_hir::PathSegment<'tcx>,
    receiver: &'tcx Expr<'tcx>,
    arguments: &'tcx [Expr<'tcx>],
    family: Family,
    adapter: Adapter,
) {
    if !movable(cx, expr, receiver) {
        return;
    }
    let Some(argument) = arguments.get(adapter.closure_argument) else {
        return;
    };
    let ExprKind::Closure(closure) = argument.kind else {
        return;
    };
    let body = cx.tcx.hir_body(closure.body);
    let Some(parameter) = body.params.get(adapter.item_parameter) else {
        return;
    };
    let Some(item) = chain::binding(parameter.pat) else {
        return;
    };
    // Each step gets its own closure after the split, so a second use of
    // the item would be left with nothing to name.
    let occurrences = uses(cx, body.value, &[item]);
    let [root] = *occurrences.as_slice() else {
        return;
    };
    // A step moves into an adapter outside the closure, where nothing the
    // body declares is in scope, so a step naming any of it would be
    // `E0425` once lifted.
    let locals = chain::declared(body);
    let steps = chain::steps(cx, root, &locals);
    if steps.len() < 2 {
        return;
    }
    // The last step stays with the adapter where the closure takes the
    // item alone, so only the ones moving out have to be liftable.
    let lifted = match adapter.keeps_the_last_step {
        true => &steps[..steps.len() - 1],
        false => &steps[..],
    };
    if !lifted.iter().all(|step| is_liftable(cx, step.expr)) {
        return;
    }
    // A parallel adapter requires the item it produces to be `Send`,
    // where the folded form does not, because the value never leaves the
    // closure.
    if family.sends_between_threads() && !lifted.iter().all(|step| is_sendable(cx, step.expr)) {
        return;
    }
    // A lifted step runs in a closure of its own, alongside the one the
    // adapter keeps. Two closures cannot both hold a capture held any way
    // but shared, so a step reaching one is declined rather than worked
    // out: whether the step is the only half reaching it is a question
    // the split's shape answers differently per adapter.
    let held_alone = exclusive(cx, closure.def_id);
    if lifted.iter().any(|step| names(cx, step.expr, &held_alone)) {
        return;
    }
    let top = steps.last().expect("two or more steps").expr;
    let anchored = match adapter.keeps_the_last_step {
        // Requiring the chain to *be* the body satisfies the position
        // condition vacuously, and holds the scope to a closure whose
        // whole job is the chain.
        true => top.hir_id == body.value.hir_id,
        false => {
            position::always_evaluated(cx, top.hir_id, body.value.hir_id)
                && position::nothing_observable_first(cx, top.hir_id, body.value.hir_id)
        }
    };
    if !anchored {
        return;
    }
    // The diagnostic span is the adapter's method segment, which a derive
    // can stamp with a user-source span, defeating both
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
            diagnostic.help(format!(
                "lift each step into a leading `{}` of its own, leaving the closure only \
                 the work that is the adapter's",
                adapter.lift_target,
            ));
        },
    );
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
/// A borrowing result is only the receiver's to answer for where nothing
/// else it was handed could have lent it: `s.max(&String::from("m")[..])`
/// returns the shorter of two lifetimes, and the shorter one belongs to a
/// temporary the closure made. So an argument that carries a region at
/// all leaves the question unanswerable, and the step declines.
///
/// Regions are erased by the time typeck results are read, so a
/// result's lifetime cannot be matched against the receiver's. These
/// clauses are the conservative answer that needs no such match.
fn is_liftable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    if !borrows(cx.typeck_results().expr_ty(step)) {
        return true;
    }
    let Some(receiver) = applied_to(step) else {
        return false;
    };
    if !cx.typeck_results().expr_ty(receiver).is_ref() {
        return false;
    }
    lends_nothing(cx, step)
}

/// Whether nothing `step` was handed besides its receiver carries a
/// region of its own.
///
/// A call's sole argument is the chain, which is the receiver here, so
/// what is left to ask about is the callee: an expression yielding an
/// `Fn` that returns a borrow lends that borrow to the result. A path to
/// a function is not one, its regions belonging to the signature rather
/// than to anything the zero-sized item holds.
fn lends_nothing<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    let lenders: Vec<&Expr<'tcx>> = match step.kind {
        ExprKind::MethodCall(_, _, arguments, _) => arguments.iter().collect(),
        ExprKind::Call(callee, _) if !matches!(callee.kind, ExprKind::Path(_)) => vec![callee],
        _ => return true,
    };
    !lenders
        .iter()
        .any(|lender| borrows(cx.typeck_results().expr_ty(lender)))
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

/// Whether `step`'s result can cross a thread boundary.
///
/// Measured on the planning file's own example:
/// `.map(|s| Rc::new(*s).len())` compiles and
/// `.map(|s| Rc::new(*s)).map(|r| r.len())` is `E0277`.
fn is_sendable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    let Some(send) = cx.tcx.get_diagnostic_item(sym::Send) else {
        return false;
    };
    implements_trait(cx, cx.typeck_results().expr_ty(step), send, &[])
}

/// Whether `ty` mentions a region, which is where a borrow would show
/// up.
fn borrows(ty: Ty<'_>) -> bool {
    ty.walk().any(|argument| argument.as_region().is_some())
}
