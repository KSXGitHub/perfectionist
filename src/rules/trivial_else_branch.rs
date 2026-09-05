use crate::common::{DefaultState, span_is_macro_generated};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Block, Expr, ExprKind, Node, QPath, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an `if` or `if let` whose `else` branch does nothing but
    /// leave — a bare `return`, `break`, or `continue`, or, when the
    /// `if` is the last expression of a function body, a bare value such
    /// as `None`, `false`, or `Ok(())` — while the branch before it holds
    /// at least `min_then_statements` (default `2`) statements, its tail
    /// expression included.
    ///
    /// Only an `if` in statement position or at the end of a block is
    /// flagged; one whose value feeds a `let` or a call is left alone,
    /// as is an `else if` chain, and an `if` produced by a macro.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Such an
    /// `if` is a guard clause written upside down: the case that
    /// leaves comes last and the real work sits indented under a
    /// condition the reader has to carry to the bottom. Inverting it —
    /// `if !ready { return; }` or `let Some(item) = next else { return; }`
    /// — puts the exit first, where the reader dismisses it, and lets
    /// the work stand at the function's own level. Every level saved
    /// this way is one the nesting rule never has to count.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::manual_let_else` (`pedantic`) rewrites a `let x = match
    /// .. { Some(x) => x, None => return }` into `let ... else`; it does
    /// not look at an `if let` with a body. `clippy::needless_else` and
    /// `clippy::redundant_else` handle an `else` after a diverging
    /// `then`, the mirror image of this shape.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// fn install(entry: Option<Entry>) {
    ///     if let Some(entry) = entry {
    ///         let target = entry.target();
    ///         fetch(&entry, &target);
    ///         link(&entry, &target);
    ///         report(&entry);
    ///     } else {
    ///         return;
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// fn install(entry: Option<Entry>) {
    ///     let Some(entry) = entry else {
    ///         return;
    ///     };
    ///     let target = entry.target();
    ///     fetch(&entry, &target);
    ///     link(&entry, &target);
    ///     report(&entry);
    /// }
    /// ```
    pub perfectionist::TRIVIAL_ELSE_BRANCH,
    Warn,
    "`else` branch only leaves; invert the condition into a guard clause",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::trivial_else_branch";

/// A branch of one expression, however many lines it wraps to, reads
/// as one expression with its `else`; inverting it buys nothing.
const DEFAULT_MIN_THEN_STATEMENTS: usize = 2;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The fewest statements, tail expression included, the branch
    /// before the `else` must hold for the `if` to be flagged. Defaults
    /// to `2`.
    min_then_statements: usize,
    /// Whether test code is left alone: an `if` inside a `#[cfg(test)]`
    /// module, a `#[test]` function, or an integration-test or
    /// benchmark target. Defaults to `false`.
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_then_statements: DEFAULT_MIN_THEN_STATEMENTS,
            test_code_exception: false,
        }
    }
}

pub struct TrivialElseBranch {
    config: Config,
}

impl_lint_pass!(TrivialElseBranch => [TRIVIAL_ELSE_BRANCH]);

impl Register for rule::TrivialElseBranch {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[TRIVIAL_ELSE_BRANCH]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(TrivialElseBranch {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

/// How the `else` branch leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exit {
    /// `return`, `break`, or `continue`: valid wherever the `if` stands.
    Diverges,
    /// A bare value: valid only as the last expression of a function
    /// body, where it is the function's return value.
    Value,
}

impl<'tcx> LateLintPass<'tcx> for TrivialElseBranch {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::If(cond, then, Some(els)) = expr.kind else {
            return;
        };
        if expr.span.desugaring_kind().is_some() || span_is_macro_generated(expr.span) {
            return;
        }
        let Some(exit) = trivial_exit(els) else {
            return;
        };
        let Some(position) = position_of(cx, expr) else {
            return;
        };
        if exit == Exit::Value && position != Position::FnBodyTail {
            return;
        }
        if then_statements(then) < self.config.min_then_statements {
            return;
        }
        if self.config.test_code_exception
            && item_in_test_code(cx, cx.tcx.hir_enclosing_body_owner(expr.hir_id))
        {
            return;
        }
        let is_if_let = matches!(cond.kind, ExprKind::Let(..));
        let message = match exit {
            Exit::Diverges => {
                "the `else` branch only leaves; write the exit first as a guard clause"
            }
            Exit::Value => {
                "the `else` branch only yields a value; return it first as a guard clause"
            }
        };
        let help = if is_if_let {
            "bind with `let ... else { <exit> };` and move the long branch after it, unindented"
        } else {
            "write `if !<condition> { <exit> }` and move the long branch after it, unindented"
        };
        span_lint_and_help(cx, TRIVIAL_ELSE_BRANCH, els.span, message, None, help);
    }
}

/// Where an `if` stands, for deciding whether an early exit keeps its
/// meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    /// A statement, or the tail of a block other than the function
    /// body's.
    Statement,
    /// The tail expression of the function body.
    FnBodyTail,
}

/// The position of `expr`, or `None` when its value is used — as a
/// `let` initialiser, an argument, an operand — so an early exit would
/// change what that use sees.
fn position_of(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<Position> {
    match cx.tcx.parent_hir_node(expr.hir_id) {
        Node::Stmt(stmt) if matches!(stmt.kind, StmtKind::Expr(_) | StmtKind::Semi(_)) => {
            Some(Position::Statement)
        }
        Node::Block(block) if block.expr.is_some_and(|tail| tail.hir_id == expr.hir_id) => {
            let block_is_fn_body = matches!(
                cx.tcx.parent_hir_node(block.hir_id),
                Node::Expr(body) if cx.enclosing_body.is_some_and(|id| cx.tcx.hir_body(id).value.hir_id == body.hir_id),
            );
            Some(if block_is_fn_body {
                Position::FnBodyTail
            } else {
                Position::Statement
            })
        }
        _ => None,
    }
}

/// How `els` leaves, when it is a block holding exactly one trivial
/// exit and nothing else.
fn trivial_exit(els: &Expr<'_>) -> Option<Exit> {
    let ExprKind::Block(block, None) = els.kind else {
        return None;
    };
    let only = sole_expr(block)?;
    match only.kind {
        ExprKind::Ret(value) => value.is_none_or(is_simple_value).then_some(Exit::Diverges),
        ExprKind::Break(_, value) => value.is_none_or(is_simple_value).then_some(Exit::Diverges),
        ExprKind::Continue(_) => Some(Exit::Diverges),
        _ => is_simple_value(only).then_some(Exit::Value),
    }
}

/// The one expression a block consists of, whether as its tail or as
/// its single statement.
fn sole_expr<'hir>(block: &'hir Block<'hir>) -> Option<&'hir Expr<'hir>> {
    match (block.stmts, block.expr) {
        ([], Some(tail)) => Some(tail),
        ([stmt], None) => match stmt.kind {
            StmtKind::Expr(expr) | StmtKind::Semi(expr) => Some(expr),
            _ => None,
        },
        _ => None,
    }
}

/// A value with nothing to compute: a literal, a path such as `None`
/// or a unit struct, the unit tuple, or a constructor call whose
/// arguments are such values (`Ok(())`, `Err(error)`, `Some(0)`).
fn is_simple_value(expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Lit(_) | ExprKind::Path(_) => true,
        ExprKind::Tup(elements) => elements.iter().all(is_simple_value),
        ExprKind::Call(callee, args) => {
            let ExprKind::Path(QPath::Resolved(None, path)) = callee.kind else {
                return false;
            };
            matches!(path.res, Res::Def(DefKind::Ctor(..), _)) && args.iter().all(is_simple_value)
        }
        _ => false,
    }
}

/// Statements in the branch before the `else`, its tail expression
/// included; a branch that is not a block is one expression.
fn then_statements(then: &Expr<'_>) -> usize {
    let ExprKind::Block(block, _) = then.kind else {
        return 1;
    };
    block.stmts.len() + usize::from(block.expr.is_some())
}
