use crate::common::{DefaultState, span_is_macro_generated};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, HirId, MatchSource};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use std::collections::HashSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the distinct method calls in one expression chain —
    /// `a.b().c().d()` is three — and flags the chain when the count
    /// is above `max_calls` (default `5`).
    ///
    /// Only method calls on the chain's spine count: the receiver of
    /// each call, down to the value the chain starts from. A run of the
    /// same method — `.arg("-v").arg("build").arg(path)` — counts once,
    /// so a builder is measured by its distinct steps. A `?` or an
    /// `.await` between two calls neither counts nor breaks the chain.
    /// A field access (`self.items.iter()` starts at `self.items`) and
    /// a function call (`Vec::new().push(1)` starts at `Vec::new()`)
    /// end the spine. A closure's body is measured on its own, so a
    /// chain inside a `map(|item| ...)` is a chain of its own. A chain
    /// produced by a macro expansion is not measured.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A long
    /// chain is a pipeline whose intermediate values have no names, so
    /// the reader has to infer what each stage produces from the method
    /// that consumes it. Up to a handful of stages that is what makes
    /// iterator code read well; past that, the reader is holding a
    /// stack of anonymous types. Binding a stage to a `let` named for
    /// what it holds, or moving a run of stages into a function named
    /// for what it does, gives the reader the name the chain withheld.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let names = manifest
    ///     .dependencies
    ///     .iter()
    ///     .filter(|(_, spec)| spec.is_workspace())
    ///     .map(|(name, _)| name.as_str())
    ///     .filter(|name| !name.starts_with('@'))
    ///     .map(str::to_owned)
    ///     .collect::<Vec<_>>()
    ///     .join(", ");
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let workspace_deps = manifest
    ///     .dependencies
    ///     .iter()
    ///     .filter(|(_, spec)| spec.is_workspace())
    ///     .map(|(name, _)| name.as_str());
    /// let unscoped: Vec<String> = workspace_deps
    ///     .filter(|name| !name.starts_with('@'))
    ///     .map(str::to_owned)
    ///     .collect();
    /// let names = unscoped.join(", ");
    /// ```
    pub perfectionist::OVERLY_LONG_METHOD_CHAIN,
    Warn,
    "expression chains more method calls than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::overly_long_method_chain";

/// Enough for `iter().filter(..).map(..).collect()` with a stage to
/// spare.
const DEFAULT_MAX_CALLS: usize = 5;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The most method calls one chain may have without being flagged.
    /// Defaults to `5`.
    max_calls: usize,
    /// Whether test code is left alone: chains inside a `#[cfg(test)]`
    /// module, a `#[test]` function, or an integration-test or
    /// benchmark target. Defaults to `false`, so a test is held to the
    /// same limit as the code it exercises.
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_calls: DEFAULT_MAX_CALLS,
            test_code_exception: false,
        }
    }
}

pub struct OverlyLongMethodChain {
    config: Config,
    /// Calls already counted as part of an outer chain, so a chain is
    /// measured once, from its head.
    counted: HashSet<HirId>,
}

impl_lint_pass!(OverlyLongMethodChain => [OVERLY_LONG_METHOD_CHAIN]);

impl Register for rule::OverlyLongMethodChain {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[OVERLY_LONG_METHOD_CHAIN]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(OverlyLongMethodChain {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                counted: HashSet::new(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for OverlyLongMethodChain {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if !matches!(expr.kind, ExprKind::MethodCall(..)) || self.counted.contains(&expr.hir_id) {
            return;
        }
        if span_is_macro_generated(expr.span) {
            return;
        }
        let count = self.count_spine(expr);
        if count <= self.config.max_calls {
            return;
        }
        if self.config.test_code_exception
            && item_in_test_code(cx, cx.tcx.hir_enclosing_body_owner(expr.hir_id))
        {
            return;
        }
        let max = self.config.max_calls;
        let noun = if count == 1 { "call" } else { "calls" };
        let message = format!("method chain has {count} {noun}, above the limit of {max}");
        span_lint_and_help(
            cx,
            OVERLY_LONG_METHOD_CHAIN,
            expr.span,
            message,
            None,
            "bind an intermediate result to a `let` named for what it holds, or move part of the chain into a function",
        );
    }
}

impl OverlyLongMethodChain {
    /// The number of method calls from `head` down its receivers, a run
    /// of one method counted once, marking each call so it is not
    /// measured again as a chain of its own.
    fn count_spine(&mut self, head: &Expr<'_>) -> usize {
        let mut count = 0;
        let mut previous_method = None;
        let mut current = head;
        loop {
            match current.kind {
                ExprKind::MethodCall(segment, receiver, ..)
                    if !span_is_macro_generated(current.span) =>
                {
                    self.counted.insert(current.hir_id);
                    if previous_method != Some(segment.ident.name) {
                        count += 1;
                    }
                    previous_method = Some(segment.ident.name);
                    current = receiver;
                }
                // `receiver?` and `receiver.await` lower to a `match` on a
                // call wrapping the receiver; the chain runs through them.
                ExprKind::Match(
                    scrutinee,
                    _,
                    MatchSource::TryDesugar(_) | MatchSource::AwaitDesugar,
                ) => {
                    let ExprKind::Call(_, [inner]) = scrutinee.kind else {
                        return count;
                    };
                    current = inner;
                }
                _ => return count,
            }
        }
    }
}
