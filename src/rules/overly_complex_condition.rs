use crate::common::{DefaultState, plural, span_is_macro_generated};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Arm, BinOpKind, Expr, ExprKind, MatchSource};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the boolean operators (`&&`, `||`) in the condition of an
    /// `if`, `if let`, `while`, `while let`, or match-arm guard, and
    /// flags the condition when the count is above `max_operators`.
    ///
    /// Only the condition itself is counted, not the branches it
    /// selects, and a closure inside the condition is a scope of its
    /// own. An `&&` with a `let` on either side of it is not counted:
    /// that `&&` is what makes the chain a chain, and no binding can
    /// replace it. A condition produced by a macro expansion is not
    /// measured, though a condition written inside a macro's arguments
    /// is. The `let` that binds a boolean is not a condition, so naming
    /// the expression is what satisfies the rule.
    ///
    /// Test code is measured like any other code; set
    /// `exempt_tests` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// condition of that many clauses is a predicate the author had
    /// in mind but did not write down; the reader has to reconstruct it
    /// from the clauses, and a later editor has to work out which
    /// clause to change. Binding the predicate, or the part of it that
    /// names a concept, to a `let` gives it the name the author had,
    /// puts a debugger-visible value on it, and turns the `if` back into
    /// a sentence. SonarSource ships this rule with the same limit.
    ///
    /// Which clauses are bound matters. A group that leads the
    /// condition runs whenever the `if` is reached, so a `let` moves it
    /// without changing when it runs. A group that follows another
    /// clause did not run when that earlier clause was false, and a
    /// `let` would make it run every time; bind a closure or a function
    /// there instead, and call it in the condition.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// if entry.is_file()
    ///     && !entry.is_hidden()
    ///     && entry.len() > 0
    ///     && entry.depth() < max_depth
    ///     && !ignored.contains(entry.path())
    /// {
    ///     copy(entry);
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let is_visible_file =
    ///     entry.is_file() && !entry.is_hidden() && entry.len() > 0;
    /// if is_visible_file
    ///     && entry.depth() < max_depth
    ///     && !ignored.contains(entry.path())
    /// {
    ///     copy(entry);
    /// }
    /// ```
    ///
    /// The group led the condition there, so the `let` runs it exactly
    /// when the `if` did. A group that follows another clause needs the
    /// lazy form, and a group that reads a binding the chain introduces
    /// takes it as a parameter:
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// if let Some(entry) = next_entry()
    ///     && entry.depth() < max_depth
    ///     && entry.is_file()
    ///     && !entry.is_hidden()
    ///     && entry.len() > 0
    ///     && !ignored.contains(entry.path())
    /// {
    ///     copy(entry);
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let is_visible_file =
    ///     |entry: &Entry| entry.is_file() && !entry.is_hidden() && entry.len() > 0;
    /// if let Some(entry) = next_entry()
    ///     && entry.depth() < max_depth
    ///     && is_visible_file(&entry)
    ///     && !ignored.contains(entry.path())
    /// {
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

/// What to do, and the constraint that decides whether it was done. The
/// count alone is satisfied by binding the clauses under any name at
/// all, so the help rules out the name a mechanical split reaches for
/// -- one assembled from the clauses rather than drawn from what they
/// together decide.
const NAMING_HELP: &str = "bind the condition, or the part of it that names a concept, to a \
                           `let` named for the predicate it decides, not for the clauses it \
                           joins";

/// The one way the fix above changes behaviour. A `let` runs its
/// initialiser where it stands, so a group lifted from anywhere but
/// the front of the condition runs even when an earlier clause would
/// have stopped it -- which a `let` chain forces, since no `let`
/// statement can sit between two clauses of the chain.
const LAZINESS_HELP: &str = "where that part follows another clause, bind a closure or a \
                             function instead, so its operands stay unevaluated until the \
                             condition reaches them";

/// Why the count can be lower than the `&&` a reader counts in the
/// source. Emitted only for a condition that has a `let` in it, which
/// is the only shape where the two differ.
const UNCOUNTED_NOTE: &str = "the `&&` that joins a `let` to the chain is not counted";

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
    exempt_tests: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_operators: DEFAULT_MAX_OPERATORS,
            exempt_tests: false,
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
        // No `hir_in_external_macro` guard. The diagnostic span is the
        // whole condition rather than a token inside it, which is the
        // case the conventions call already covered: a composite span
        // carries the expansion's context, so
        // `report_in_external_macro: false` filters another crate's
        // expansion, and `span_is_macro_generated` below stops a
        // `macro_rules!` in this one, which that flag does not reach.
        if span_is_macro_generated(condition.span) {
            return;
        }
        let operators = count_boolean_operators(condition);
        let count = operators.count;
        if count <= self.config.max_operators {
            return;
        }
        if self.config.exempt_tests
            && item_in_test_code(cx, cx.tcx.hir_enclosing_body_owner(condition.hir_id))
        {
            return;
        }
        let max = self.config.max_operators;
        let noun = plural(count, "operator", "operators");
        let message = format!("condition has {count} boolean {noun}, above the limit of {max}");
        span_lint_and_then(
            cx,
            OVERLY_COMPLEX_CONDITION,
            condition.span,
            message,
            |diag| {
                diag.help(NAMING_HELP);
                diag.help(LAZINESS_HELP);
                if operators.binds_a_pattern {
                    diag.note(UNCOUNTED_NOTE);
                }
            },
        );
    }
}

/// What a condition's operators add up to.
struct Operators {
    /// The number of `&&` and `||` operators in the condition, outside
    /// any nested body and outside macro expansions, and outside the
    /// `&&`s that hold a `let` chain together.
    count: usize,
    /// Whether any clause of the chain is a `let`, which is what makes
    /// the uncounted `&&`s worth a note on the diagnostic: the reader
    /// can see more `&&` in the source than the count names.
    binds_a_pattern: bool,
}

/// The `&&`-joined clauses of `expr`, left to right.
///
/// A macro expansion is one clause however it is shaped, because the
/// `&&` inside it is not the author's: the chain ends where the
/// expansion begins.
fn and_chain_clauses<'tcx>(expr: &'tcx Expr<'tcx>, clauses: &mut Vec<&'tcx Expr<'tcx>>) {
    if let ExprKind::Binary(op, lhs, rhs) = expr.kind
        && op.node == BinOpKind::And
        && !span_is_macro_generated(expr.span)
    {
        and_chain_clauses(lhs, clauses);
        and_chain_clauses(rhs, clauses);
    } else {
        clauses.push(expr);
    }
}

fn is_let(expr: &Expr<'_>) -> bool {
    matches!(expr.kind, ExprKind::Let(..))
}

/// The operators of `condition` that a `let` binding could remove.
///
/// The condition is flattened into the clauses its top-level `&&`s
/// join, which is the only place a `let` may appear. Each gap between
/// two clauses is one `&&` the author wrote, and it counts unless a
/// `let` sits on either side of it: a group of ordinary clauses
/// collapses into one named clause, taking its `&&`s with it, whereas
/// an `&&` next to a `let` is what makes the chain a chain. The
/// operators *inside* each clause -- a `||`, a parenthesised `&&` --
/// are counted the ordinary way, since no `let` can be nested there.
///
/// [`OperatorCounter`] leaves `NestedFilter` at its `None` default, so
/// `walk_expr` never descends into a closure body or a `const` block.
/// That is what keeps a closure's operators out of the enclosing
/// condition, not an arm of the match below.
fn count_boolean_operators<'tcx>(condition: &'tcx Expr<'tcx>) -> Operators {
    let mut clauses = Vec::new();
    and_chain_clauses(condition, &mut clauses);

    let mut count = clauses
        .windows(2)
        .filter(|pair| !is_let(pair[0]) && !is_let(pair[1]))
        .count();
    for clause in &clauses {
        let mut counter = OperatorCounter { count: 0 };
        counter.visit_expr(clause);
        count += counter.count;
    }

    Operators {
        count,
        binds_a_pattern: clauses.iter().copied().any(is_let),
    }
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
            // A nested `if` or `match` written inside a condition is
            // not part of it. Its branches are code the outer
            // condition selects between rather than tests, and its own
            // head is a condition in its own right, which `check_expr`
            // and `check_arm` reach separately -- without this, the
            // `&&` in `if a && (if b && c { d } else { e })` is counted
            // once here and again there. Only an author-written
            // `match` stops the walk: `?` and `.await` also lower to
            // one, and stepping over those would skip the expression
            // they wrap.
            ExprKind::If(..) | ExprKind::Match(_, _, MatchSource::Normal) => {}
            ExprKind::Binary(op, ..) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
                self.count += 1;
                intravisit::walk_expr(self, expr);
            }
            _ => intravisit::walk_expr(self, expr),
        }
    }
}
