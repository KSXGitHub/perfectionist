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
use rustc_errors::Applicability;
use rustc_hir::{Closure, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};

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
    /// A chain the closure does not always reach stays, since lifting it
    /// would run it for every item rather than for some. An item the
    /// closure names more than once stays, there being no binding for the
    /// later mentions to name once each step has its own closure.
    ///
    /// A conjunction of nothing but comparisons stays whole, because that
    /// is how Rust spells one test: `pos >= range.start && pos < range.end`
    /// asks whether a position is inside a range, and each half on its own
    /// adapter reads worse than the pair does. One comparison among named
    /// questions is not that shape, and does split.
    ///
    /// Otherwise the lint declines any split that would not compile, or
    /// would not mean what the closure means.
    ///
    /// ### Applicability
    ///
    /// A conjunction is rewritten, one adapter per test. The other two
    /// shapes are described rather than rewritten, their splits having
    /// more than one reasonable text.
    ///
    /// The rewrite keeps the closure each test was written in rather than
    /// reducing it to a path. A path asks more of a test than the split
    /// does, so `clippy::redundant_closure_for_method_calls` is what
    /// reduces the ones that can be reduced, and the two fixes compose in
    /// that order.
    ///
    /// A rewrite that would drop a comment the predicate holds is offered
    /// as advice rather than applied.
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
    /// Whether the split hands the receiver to an adapter taking it by
    /// value, which is what [`movable`] answers for. A split whose leading
    /// adapter borrows the receiver as the folded one does needs no move
    /// taken from it.
    moves_the_receiver: bool,
    /// The rewrite, where the trigger can write one.
    fix: Option<Fix>,
}

/// A rewrite the reader can take as it stands.
///
/// The span runs from the adapter's own name to the end of its call, so the
/// receiver is never reproduced: whatever it is, however long, it stays
/// where it is and the text replaces only what follows the dot.
///
/// Every suggestion keeps the closures the folded form had rather than
/// reducing them to paths. A path asks more of a step than the split does,
/// `.map(Path::components)` being `E0631` where `.map(|p| p.components())`
/// compiles, and `clippy::redundant_closure_for_method_calls` is the lint
/// that reduces the ones that can be reduced. Nothing in Clippy asks for
/// the closure form, so the two fixes compose in that order and stop.
struct Fix {
    /// What to replace, which is the adapter's name through its closing
    /// bracket.
    span: Span,
    /// What to put there, which ends in the adapter the folded form had.
    suggestion: String,
    /// `MachineApplicable` only where the text is known to compile and to
    /// behave as the folded form did.
    applicability: Applicability,
}

/// The adapter call a trigger reads, so each one is handed the parts
/// rather than destructuring the same expression again.
struct Call<'tcx> {
    /// The adapter's own name.
    method: Symbol,
    /// Its arguments, among which is the closure.
    arguments: &'tcx [Expr<'tcx>],
    /// What a [`Fix`] replaces: the adapter's name through the end of its
    /// call, which leaves the receiver where it is.
    fix_span: Span,
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
        let call = Call {
            method: segment.ident.name,
            arguments,
            fix_span: segment.ident.span.with_hi(expr.span.hi()),
        };
        let found = option_chain::check(cx, &call, family)
            .or_else(|| predicate::check(cx, &call, family))
            .or_else(|| step_chain::check(cx, &call, family));
        let Some(found) = found else {
            return;
        };
        // A split lifting work into an adapter that takes the iterator by
        // value, where the adapter it came from need not, needs a receiver
        // a move can be taken from.
        if found.moves_the_receiver && !movable(cx, expr, receiver) {
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
            |diagnostic| match found.fix {
                // The help text is the suggestion's own label, so a reader
                // seeing the rewrite reads why it is that shape.
                Some(fix) => {
                    diagnostic.span_suggestion(
                        fix.span,
                        found.help,
                        fix.suggestion,
                        fix.applicability,
                    );
                }
                None => {
                    diagnostic.help(found.help);
                }
            },
        );
    }
}
