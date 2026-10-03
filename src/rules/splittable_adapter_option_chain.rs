//! `perfectionist::splittable_adapter_option_chain` — flag a closure
//! passed to an `Option`-returning adapter that welds a guard to a
//! value, so one adapter does both.
//!
//! `filter_map` and its kin split on neither of the other triggers'
//! terms. Their closure returns an `Option`, so there is no conjunction
//! to cut, and a leading `map` would hand the next adapter an `Option`
//! rather than a value. What splits is the `Option` work inside: each
//! combinator has an iterator adapter that does the same thing, and the
//! split hands the work over.
//!
//! Which adapter it hands it to depends on the outer one's discipline,
//! which [`crate::adapter_discipline`] is about. A lifted `filter_map`
//! in front of a `map_while` would drop the very item that would have
//! stopped it, so a prefix-shaped adapter takes only the forms whose
//! lift target filters its way or not at all.

use self::combinator::Yield;
use crate::adapter_discipline::Discipline;
use crate::common::{DefaultState, binding_hir_id, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::{is_from_proc_macro, sym};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

mod combinator;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a closure passed to `filter_map`, `find_map` or `map_while`
    /// whose body ends in an `Option` combinator or a `bool::then`, so
    /// one adapter performs both the step that produced the `Option` and
    /// the one applied to it.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Both
    /// forms keep the same items, in the same order. A `then_some` is
    /// the one that does less work split than folded, since it builds
    /// its value whether the guard holds or not; the split can drop a
    /// side effect there, never add one.
    ///
    /// The preference is that each adapter does one thing, so a reader
    /// sees where the guard is and where the value is made without
    /// parsing a closure to separate them. `filter_map(parse)` followed
    /// by `filter_map(validate)` says there are two fallible stages;
    /// folded into one closure, the same two have to be read out of an
    /// expression.
    ///
    /// ### When it stays silent
    ///
    /// A closure whose `Option` work is one step has nothing to split.
    /// A `map_while` takes only the forms whose lift target shares its
    /// discipline: a leading `filter_map` in front of it would drop the
    /// item that would have stopped it, so only a `then` guard, which
    /// lifts into a `take_while`, and a trailing `map` split there.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let found = lines.filter_map(|line| parse(line).and_then(validate));
    /// let shown = lines.filter_map(|line| wanted(line).then(|| render(line)));
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let found = lines.filter_map(parse).filter_map(validate);
    /// let shown = lines.filter(wanted).map(render);
    /// ```
    pub perfectionist::SPLITTABLE_ADAPTER_OPTION_CHAIN,
    Warn,
    "a closure passed to an `Option`-returning adapter welds a guard to a value",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::splittable_adapter_option_chain";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct SplittableAdapterOptionChain;

impl_lint_pass!(SplittableAdapterOptionChain => [SPLITTABLE_ADAPTER_OPTION_CHAIN]);

impl Register for rule::SplittableAdapterOptionChain {
    /// A `filter_map` closure ending in a combinator is ordinary
    /// idiomatic Rust, so the rule fires often. That is a reason to
    /// expect diagnostics rather than a reason to ship the rule off.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SPLITTABLE_ADAPTER_OPTION_CHAIN]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(SplittableAdapterOptionChain)));
    }
}

/// The discipline of `method`'s answer and whether it yields a stream,
/// or `None` for a method this rule does not speak about.
///
/// Yielding a stream is a second question the discipline does not
/// answer. `find_map` returns one value, so the only trailing adapter it
/// has is the `Option`'s own: a trailing `filter` there is
/// `Option::filter`, which compiles and can only reject what the search
/// already settled on, where the folded form kept looking.
fn adapter(method: Symbol) -> Option<(Discipline, Yield)> {
    Some(match method.as_str() {
        "filter_map" => (Discipline::Set, Yield::Stream),
        "find_map" => (Discipline::Set, Yield::OneValue),
        "map_while" => (Discipline::Prefix, Yield::Stream),
        _ => return None,
    })
}

impl<'tcx> LateLintPass<'tcx> for SplittableAdapterOptionChain {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(segment, _, arguments, _) = expr.kind else {
            return;
        };
        let Some((discipline, yields)) = adapter(segment.ident.name) else {
            return;
        };
        let Some(method) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
            return;
        };
        if !cx
            .tcx
            .trait_of_assoc(method)
            .is_some_and(|declaring| cx.tcx.is_diagnostic_item(sym::Iterator, declaring))
        {
            return;
        }
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
        let Some(split) = combinator::split(cx, body.value, item, item_ty, discipline, yields)
        else {
            return;
        };
        if segment.ident.span.from_expansion()
            || hir_in_external_macro(cx, expr.hir_id, segment.ident.span)
            || is_from_proc_macro(cx, expr)
        {
            return;
        }
        span_lint_and_then(
            cx,
            SPLITTABLE_ADAPTER_OPTION_CHAIN,
            segment.ident.span,
            format!(
                "this closure holds a guard and a value, so `{}` does both",
                segment.ident.name,
            ),
            |diagnostic| {
                diagnostic.help(split.help());
            },
        );
    }
}
