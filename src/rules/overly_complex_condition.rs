use crate::common::{DefaultState, span_is_macro_generated};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Arm, BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the boolean operators (`&&`, `||`) in the condition of an
    /// `if`, `if let`, `while`, `while let`, or match-arm guard, and
    /// flags the condition when the count is above `max_operators`
    /// (default `3`).
    ///
    /// Only the condition itself is counted, not the branches it
    /// selects, and a closure inside the condition is a scope of its
    /// own. The `&&` that joins the `let`s of a `let` chain counts like
    /// any other. A condition produced by a macro expansion is not
    /// measured, though a condition written inside a macro's arguments
    /// is. The `let` that binds a boolean is not a condition, so naming
    /// the expression is what satisfies the rule.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// condition of four or more clauses is a predicate the author had
    /// in mind but did not write down; the reader has to reconstruct it
    /// from the clauses, and a later editor has to work out which
    /// clause to change. Binding the predicate, or the part of it that
    /// names a concept, to a `let` gives it the name the author had,
    /// puts a debugger-visible value on it, and turns the `if` back into
    /// a sentence. SonarSource ships this rule with the same limit.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// if entry.is_file() && !entry.is_hidden() && entry.len() > 0 && !ignored.contains(entry.path()) {
    ///     copy(entry);
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let is_visible_file = entry.is_file() && !entry.is_hidden() && entry.len() > 0;
    /// if is_visible_file && !ignored.contains(entry.path()) {
    ///     copy(entry);
    /// }
    /// ```
    pub perfectionist::OVERLY_COMPLEX_CONDITION,
    Warn,
    "condition has more boolean operators than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::overly_complex_condition";

/// SonarSource's limit for the same rule.
const DEFAULT_MAX_OPERATORS: usize = 3;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The most `&&` and `||` operators a condition may have without
    /// being flagged. Defaults to `3`.
    max_operators: usize,
    /// Whether test code is left alone: conditions inside a
    /// `#[cfg(test)]` module, a `#[test]` function, or an
    /// integration-test or benchmark target. Defaults to `false`, so a
    /// test is held to the same limit as the code it exercises.
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_operators: DEFAULT_MAX_OPERATORS,
            test_code_exception: false,
        }
    }
}

pub struct OverlyComplexCondition {
    config: Config,
}

impl_lint_pass!(OverlyComplexCondition => [OVERLY_COMPLEX_CONDITION]);

impl Register for rule::OverlyComplexCondition {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[OVERLY_COMPLEX_CONDITION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(OverlyComplexCondition {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for OverlyComplexCondition {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // `while` lowers to a `loop` around an `if` whose condition is
        // still the author's, so it needs no case of its own.
        if let ExprKind::If(condition, ..) = expr.kind {
            self.check_condition(cx, condition);
        }
    }

    fn check_arm(&mut self, cx: &LateContext<'tcx>, arm: &'tcx Arm<'tcx>) {
        if let Some(guard) = arm.guard {
            self.check_condition(cx, guard);
        }
    }
}

impl OverlyComplexCondition {
    fn check_condition<'tcx>(&self, cx: &LateContext<'tcx>, condition: &'tcx Expr<'tcx>) {
        if span_is_macro_generated(condition.span) {
            return;
        }
        let count = count_boolean_operators(condition);
        if count <= self.config.max_operators {
            return;
        }
        if self.config.test_code_exception
            && item_in_test_code(cx, cx.tcx.hir_enclosing_body_owner(condition.hir_id))
        {
            return;
        }
        let max = self.config.max_operators;
        let noun = if count == 1 { "operator" } else { "operators" };
        let message = format!("condition has {count} boolean {noun}, above the limit of {max}");
        span_lint_and_help(
            cx,
            OVERLY_COMPLEX_CONDITION,
            condition.span,
            message,
            None,
            "bind the condition, or the part of it that names a concept, to a `let`",
        );
    }
}

/// The number of `&&` and `||` operators in `condition`, outside any
/// closure and outside macro expansions.
fn count_boolean_operators<'tcx>(condition: &'tcx Expr<'tcx>) -> usize {
    let mut counter = OperatorCounter { count: 0 };
    counter.visit_expr(condition);
    counter.count
}

struct OperatorCounter {
    count: usize,
}

impl<'tcx> Visitor<'tcx> for OperatorCounter {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if span_is_macro_generated(expr.span) {
            return;
        }
        match expr.kind {
            ExprKind::Closure(_) => {}
            ExprKind::Binary(op, ..) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
                self.count += 1;
                intravisit::walk_expr(self, expr);
            }
            _ => intravisit::walk_expr(self, expr),
        }
    }
}
