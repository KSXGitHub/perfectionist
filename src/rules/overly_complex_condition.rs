use crate::common::{
    DefaultState, and_chain_clauses, expr_is_let, is_author_written_match, plural,
    span_is_macro_generated,
};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Arm, BinOpKind, Expr, ExprKind};
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
    /// own. An `&&` with a `let` on either side of it is not counted,
    /// since no binding can replace it. A condition produced by a macro
    /// expansion is not measured, though a condition written inside a
    /// macro's arguments is. The `let` that binds a boolean is not a
    /// condition, so naming the expression is what satisfies the rule.
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
    /// a sentence.
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
    /// **Prefer:** a leading part, which a `let` runs exactly when the
    /// `if` would have
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
    /// **Avoid:** a part that follows a clause and reads a binding from
    /// the chain
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
    /// **Prefer:** a closure, which stays unevaluated until the
    /// condition reaches it
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

/// When the fix above is the wrong one, in the shape
/// `too_many_struct_fields` and `excessive_nesting` use for their
/// second help. A `let` chain forces the case, since no `let`
/// statement can sit between two clauses of a chain, but it is not
/// confined to one: any part that does not lead its condition is
/// reached only when the clauses before it hold.
const LAZINESS_HELP: &str = "where that part follows another clause, bind a closure or a \
                             function instead, so its clauses stay unevaluated until the \
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_operators: DEFAULT_MAX_OPERATORS,
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
                if operators.omits_a_join {
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
    /// Whether a join between two clauses went uncounted, which is
    /// what makes the note worth emitting: the reader can then see more
    /// `&&` in the source than the count names. A `let` elsewhere in
    /// the condition -- in a `let`'s own initialiser, say -- omits
    /// nothing, so it earns no note.
    omits_a_join: bool,
}

/// The operators of `condition` that a `let` binding could remove.
///
/// The condition is flattened into the clauses its top-level `&&`s
/// join. Each join between two clauses is one `&&` the author wrote,
/// and it counts unless a
/// `let` sits on either side of it: a run of ordinary clauses
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
    let clauses = and_chain_clauses(condition);

    let counted_joins = clauses
        .windows(2)
        .filter(|pair| !expr_is_let(pair[0]) && !expr_is_let(pair[1]))
        .count();
    let mut count = counted_joins;
    for clause in &clauses {
        let mut counter = OperatorCounter { count: 0 };
        counter.visit_expr(clause);
        count += counter.count;
    }

    Operators {
        count,
        omits_a_join: counted_joins < clauses.len().saturating_sub(1),
    }
}

struct OperatorCounter {
    count: usize,
}

impl<'tcx> Visitor<'tcx> for OperatorCounter {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        // A nested `if` or `match` is not part of the condition it sits
        // in. Its branches are code that condition selects between
        // rather than tests, and its own head is a condition in its own
        // right, which `check_expr` and `check_arm` reach separately --
        // without this, the `&&` in `if a && (if b && c { d } else { e })`
        // is counted once here and again there. A `match` scrutinee is
        // the exception: the condition does evaluate it, and no other
        // pass reaches it the way `check_expr` reaches a nested `if`
        // head, so it is walked rather than skipped with the arms.
        //
        // This is decided before the node's provenance, because a macro
        // can expand to either construct around the author's arguments:
        // in `if first && make_if!(second && third)` the generated `if`
        // still selects between branches, and its head is still reached
        // on its own, so stepping into it would count `second && third`
        // twice.
        match expr.kind {
            ExprKind::If(..) => return,
            ExprKind::Match(scrutinee, _, source) if is_author_written_match(source) => {
                self.visit_expr(scrutinee);
                return;
            }
            _ => {}
        }
        // The node is the macro's, but an argument expression keeps its
        // call-site span, so the subtree can still hold operators the
        // author wrote. Step over this node without counting it rather
        // than pruning what hangs below it, as
        // `excessive_cognitive_complexity` does.
        if span_is_macro_generated(expr.span) {
            intravisit::walk_expr(self, expr);
            return;
        }
        match expr.kind {
            ExprKind::Binary(op, ..) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
                self.count += 1;
                intravisit::walk_expr(self, expr);
            }
            _ => intravisit::walk_expr(self, expr),
        }
    }
}
