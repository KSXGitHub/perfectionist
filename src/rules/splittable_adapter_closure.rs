//! `perfectionist::splittable_adapter_closure` — flag a closure passed to
//! an adapter that does several things the pipeline has an adapter apiece
//! for, so one adapter does all of them.
//!
//! One lint over three triggers, because what the closure holds decides
//! which split it has rather than which adapter holds it. [`step_chain`]
//! reads a chain of steps rooted at the item, [`predicate`] a conjunction
//! of tests, and [`option_chain`] an `Option` combinator welding a guard
//! to a value. [`mod@family`] answers which receiver they are all on.
//!
//! The triggers are asked in order and the first finding wins. They
//! overlap on the adapters whose closure returns an `Option`: a body that
//! is `Option` work splits into the combinator's counterpart, and one
//! that is a chain splits into a leading `map`, so the finer split is
//! asked for first.
//!
//! Scope: steps and tests taking their receiver by value, over the
//! families [`family::Family`] names. `planned-rules/splittable-adapter-closure.md`
//! records what of the planning file is left.

use self::family::family;
use crate::common::{DefaultState, hir_in_external_macro};
use crate::receiver_move::movable;
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::is_from_proc_macro;
use rustc_hir::{Closure, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod adapter;
mod anchoring;
mod chain;
mod combinator;
mod family;
mod option_chain;
mod predicate;
mod step_chain;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a closure passed to an adapter where the closure does
    /// several things the pipeline has an adapter apiece for, so one
    /// adapter does all of them. Three shapes trigger it:
    ///
    /// - A chain of two or more steps rooted at the closure's item, where
    ///   each step is a leading `map` of its own. The receivers in scope
    ///   are iterators, `Option`, `Result`, `Poll`, `ControlFlow`, and
    ///   the `Itertools`, `ParallelIterator` and `Pipe` traits.
    /// - A predicate that is a conjunction of two or more tests, each
    ///   naming the item, where each test is a filtering adapter of its
    ///   own.
    /// - A body under `filter_map`, `find_map` or `map_while` that welds
    ///   a guard to a value or runs two fallible stages, where each half
    ///   is the adapter that does that half.
    ///
    /// A chain starts at the item and runs outwards: a method call whose
    /// receiver is the chain so far, or a call whose sole argument is it,
    /// naming nothing else the closure declares.
    ///
    /// The adapters a chain splits under are the ones whose item enters
    /// the closure by value and never comes back out, which is what lets
    /// a leading `map` mean the same thing. `filter`, `take_while`,
    /// `inspect` and their kin hold a chain alone, because a `map` in
    /// front of them changes what they see.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Every
    /// split keeps the same items, in the same order, with the same
    /// laziness and the same short-circuiting, and a closure that does
    /// two things is ordinary idiomatic Rust.
    ///
    /// The preference is that a pipeline written one thing per adapter can
    /// be read, cut and instrumented at every one of them. An `inspect`
    /// can go between any two, one can be deleted by deleting a line, and
    /// the reader sees how many things the data goes through without
    /// parsing an expression to count them. Folded into a closure, the
    /// same things have to be read out of that expression first.
    ///
    /// ### When it stays silent
    ///
    /// A closure doing one thing has nothing to split. A capture that
    /// only one half of the split could hold is left alone, since the two
    /// closures the split writes both reach it, which a shared borrow and
    /// a copy of a `Copy` value are the cases that survive.
    ///
    /// Three are the chain's own. An item named more than once cannot be
    /// split at all, since each step would get its own closure and the
    /// later mentions would have no binding to name. A step whose result
    /// borrows lifts only where it borrows through a reference it was
    /// handed, so one borrowing an owned item, or a temporary the closure
    /// made, stays. And a chain the closure does not always reach is left
    /// alone, because lifting it would run it for every item rather than
    /// for some.
    ///
    /// Four are about the tests. A disjunction does not split: filtering
    /// on one of two alternatives keeps items the pair would have
    /// dropped. An adapter whose answer depends on more than which items
    /// satisfy the test has nothing to lift into, so `all`, `position`
    /// and their kin are left alone: filtering before `all` makes an item
    /// that failed the first test vacuously fine. A conjunct not naming
    /// the item is an invariant to hoist out of the pipeline rather than a
    /// test to give its own adapter. And a conjunction of nothing but
    /// comparisons stays whole, because that is how Rust spells one test:
    /// `pos >= range.start && pos < range.end` asks whether a position is
    /// inside a range, and each half on its own adapter reads worse than
    /// the pair does. One comparison among named questions is not that
    /// shape, and does split.
    ///
    /// Two are about what the split would not compile into. `any` and
    /// `Option::is_some_and` hand the item over, where the `filter` a
    /// test lifts into hands a reference, so a test writing to the item
    /// or handing it to something wanting the value is left alone. And a
    /// `map_while` takes only the forms whose lift target shares its
    /// discipline: a leading `filter_map` in front of it would drop the
    /// item that would have stopped it.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let names = headers.map(|header| header.trim().to_ascii_lowercase());
    /// let found = paths.filter(|path| path.is_file() && path.exists());
    /// let shown = lines.filter_map(|line| wanted(line).then(|| render(line)));
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let names = headers.map(str::trim).map(str::to_ascii_lowercase);
    /// let found = paths.filter(|path| path.is_file()).filter(|path| path.exists());
    /// let shown = lines.filter(|line| wanted(line)).map(render);
    /// ```
    pub perfectionist::SPLITTABLE_ADAPTER_CLOSURE,
    Warn,
    "a closure passed to an adapter does several things the pipeline has an adapter apiece for",
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
    /// Every shape the rule flags is ordinary idiomatic Rust, so it fires
    /// often. That is a reason to expect diagnostics rather than a reason
    /// to ship the rule off: firing on what the rule names is not a false
    /// positive.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SPLITTABLE_ADAPTER_CLOSURE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(SplittableAdapterClosure)));
    }
}

/// What a trigger found. The span is the method segment for all three, so
/// a finding carries the text alone.
struct Finding {
    /// What the closure is doing twice.
    message: String,
    /// What to do about it, which differs by what the split hands the
    /// work to.
    help: String,
}

/// The closure an adapter holds, where it holds exactly one.
///
/// An adapter taking one closure per channel is read by position
/// instead, since its name alone does not say which channel a closure is
/// on.
fn only_closure<'tcx>(arguments: &'tcx [Expr<'tcx>]) -> Option<&'tcx Closure<'tcx>> {
    arguments.iter().find_map(|argument| match argument.kind {
        ExprKind::Closure(closure) => Some(closure),
        _ => None,
    })
}

impl<'tcx> LateLintPass<'tcx> for SplittableAdapterClosure {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(segment, receiver, arguments, _) = expr.kind else {
            return;
        };
        let Some(family) = family(cx, expr, receiver) else {
            return;
        };
        let method = segment.ident.name;
        let found = option_chain::check(cx, method, arguments, family)
            .or_else(|| predicate::check(cx, method, arguments, family))
            .or_else(|| step_chain::check(cx, method, arguments, family));
        let Some(found) = found else {
            return;
        };
        // Every split lifts work into an adapter taking the iterator by
        // value, where the adapter it came from need not, so the receiver
        // has to be one a move can be taken from.
        if !movable(cx, expr, receiver) {
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
            found.message,
            |diagnostic| {
                diagnostic.help(found.help);
            },
        );
    }
}
