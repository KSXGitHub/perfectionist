use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

mod callback;
mod statements;

use callback::iterator_callback;
use statements::{StatementCounts, count_source_statements};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags inline callbacks to standard `Iterator` methods whose bodies
    /// contain more top-level statements than `max_statements` permits.
    /// A tail expression counts as a statement. Comments, line wrapping,
    /// and empty semicolons do not count. A single expression stays one even
    /// when it contains a `match`, an `if`, or a nested block.
    ///
    /// The check covers transforming, filtering, visiting and reducing
    /// callbacks, in method calls and qualified calls such as `Iterator::map`.
    /// Named callbacks, custom methods with the same names, parallel
    /// iterators, asynchronous streams, and macro-generated closures are
    /// left alone. Callbacks inside macro arguments are also left alone.
    /// A macro invocation inside a checked callback counts once, regardless
    /// of its expansion. Test code follows the same policy as production code.
    ///
    /// ### Why restrict this?
    ///
    /// A short callback keeps the transformation visible in the chain.
    /// A callback that performs several steps can be easier to follow as
    /// a named helper, or as a loop body when the operation is eager.
    /// This is an opt-in readability preference, not a correctness check.
    ///
    /// Extracting a helper preserves a chain's structure. A loop rewrite
    /// needs more care: preserve laziness, evaluation order, ownership,
    /// allocation behavior, and short-circuit results. A closure-local
    /// `return` must not become a return from the enclosing function.
    /// The lint does not establish equivalent performance and provides no
    /// automatic fix. Benchmark performance-sensitive rewrites.
    ///
    /// ### Interaction with Clippy
    ///
    /// Clippy's `needless_for_each` already recommends loops for simple
    /// eager iteration. This check also covers callbacks inside transformed
    /// or lazy chains. The two can report the same `for_each` call: Clippy
    /// checks the iteration form, while this rule checks the callback body.
    ///
    /// ### Example
    ///
    /// ```rust,ignore
    /// entries.iter().map(|entry| {
    ///     let name = normalize(entry.name());
    ///     format!("{name}: {}", entry.value())
    /// })
    /// ```
    ///
    /// Keep the lazy chain and name the operation:
    ///
    /// ```rust,ignore
    /// entries.iter().map(format_entry)
    /// ```
    ///
    /// A multiline single expression is also allowed:
    ///
    /// ```rust,ignore
    /// entries.iter().map(|entry| match entry.kind() {
    ///     Kind::File => entry.file_name(),
    ///     Kind::Directory => entry.directory_name(),
    /// })
    /// ```
    pub perfectionist::MULTI_STATEMENT_ITERATOR_CLOSURE,
    Warn,
    "iterator callback has too many top-level statements",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::multi_statement_iterator_closure";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    /// Maximum top-level statements, including a tail expression, in an
    /// inline iterator callback. Defaults to `1`.
    max_statements: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self { max_statements: 1 }
    }
}

pub struct MultiStatementIteratorClosure {
    config: Config,
    counts: StatementCounts,
}

impl_lint_pass!(MultiStatementIteratorClosure => [MULTI_STATEMENT_ITERATOR_CLOSURE]);

impl Register for rule::MultiStatementIteratorClosure {
    // Projects differ on whether several steps belong inline or in a named helper.
    const DEFAULT_STATE: DefaultState = DefaultState::Inactive;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MULTI_STATEMENT_ITERATOR_CLOSURE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(MultiStatementIteratorClosure {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                counts: StatementCounts::new(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for MultiStatementIteratorClosure {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.counts = count_source_statements(cx);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Closure(_) = expr.kind else {
            return;
        };
        let Node::Expr(call) = cx.tcx.parent_hir_node(expr.hir_id) else {
            return;
        };
        let Some((method, callback)) = iterator_callback(cx, call) else {
            return;
        };
        if callback.hir_id != expr.hir_id {
            return;
        }
        if callback.span.from_expansion()
            || hir_in_external_macro(cx, callback.hir_id, callback.span)
        {
            return;
        }
        let Some(statements) = self
            .counts
            .get(&(callback.span.lo(), callback.span.hi()))
            .copied()
        else {
            return;
        };
        if statements > self.config.max_statements {
            emit(cx, callback, method, statements, self.config.max_statements);
        }
    }
}

fn emit(cx: &LateContext<'_>, callback: &Expr<'_>, method: Symbol, statements: usize, max: usize) {
    let unit = if statements == 1 {
        "statement"
    } else {
        "statements"
    };
    span_lint_and_then(
        cx,
        MULTI_STATEMENT_ITERATOR_CLOSURE,
        callback.span,
        format!("`{method}` callback has {statements} top-level {unit}, above the limit of {max}"),
        |diag| {
            diag.help("extract a named helper, or use an explicit loop for eager work when it preserves the required behavior and performance");
            diag.note("a helper can preserve a lazy chain; loop rewrites must preserve evaluation order, ownership, allocation behavior and short-circuit results");
        },
    );
}
