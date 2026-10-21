use crate::common::{DefaultState, plural};
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
    /// Flags an inline callback to a standard `Iterator` method whose
    /// body does more than `max_statements` permits. The value the body
    /// produces counts towards that. A `let` and the value it feeds are
    /// already two.
    ///
    /// Enable the rule per crate by adding to `dylint.toml`:
    ///
    /// ```toml
    /// [perfectionist]
    /// enable = ["multi_statement_iterator_closure"]
    /// ```
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A short
    /// callback keeps the transformation visible in the chain. A callback
    /// that performs several steps can be easier to follow as a named
    /// helper. When the operation is eager, a loop body can be easier
    /// still.
    ///
    /// A loop rewrite needs more care than a helper does. A
    /// closure-local `return` must not become a return from the
    /// enclosing function. The rule establishes nothing about
    /// performance. Benchmark a rewrite that matters to it.
    ///
    /// ### Interaction with Clippy
    ///
    /// Clippy's `needless_for_each` already recommends loops for simple
    /// eager iteration. This check also covers callbacks inside transformed
    /// or lazy chains. The two can report the same `for_each` call.
    /// Clippy checks the iteration form. This rule checks the callback
    /// body.
    ///
    /// ### Example
    ///
    /// **Avoid:** 2 top-level statements
    ///
    /// ```rust,ignore
    /// entries.iter().map(|entry| {
    ///     let name = normalize(entry.name());
    ///     format!("{name}: {}", entry.value())
    /// })
    /// ```
    ///
    /// **Prefer:** a name for the operation
    ///
    /// ```rust,ignore
    /// entries.iter().map(format_entry)
    ///
    /// fn format_entry(entry: &Entry) -> String {
    ///     let name = normalize(entry.name());
    ///     format!("{name}: {}", entry.value())
    /// }
    /// ```
    ///
    /// **Not flagged:** one expression, however many lines it spans
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
    /// Maximum top-level statements in an inline iterator callback. The
    /// value the body produces counts as one. Defaults to `1`.
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
        if callback.span.from_expansion() {
            return;
        }
        // The lookup is what establishes that the author wrote this
        // callback, and writes it to one of the methods the rule
        // covers. `count_source_statements` records a range only for a
        // closure the source shows in that position. A call a macro
        // assembled around the author's closure therefore finds
        // nothing here. No question about where either span came from
        // has to stand in for reading the source.
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

const EXTRACTION_HELP: &str = "extract a named helper, or use an explicit loop for eager work \
                               when it preserves the required behaviour and performance";

/// How to tell the fix did not work. The sibling threshold rules put
/// a test the reader applies to the result, and this follows them.
/// The shapes this rule leaves alone are a closure bound to a `let`
/// and passed by name, and a body wrapped in one more block. Naming
/// either of them here would print a working bypass under a `help:`
/// label. There it reads as a recipe rather than as a caution.
const MOVED_HELP: &str = "if the only name that fits the helper describes the steps rather than \
                          the value it produces (`double_and_increment`, `parse_then_insert`), \
                          the steps moved rather than went away";

const REWRITE_NOTE: &str = "a helper can preserve a lazy chain; loop rewrites must preserve \
                            evaluation order, ownership, allocation behaviour and short-circuit \
                            results";

fn emit(cx: &LateContext<'_>, callback: &Expr<'_>, method: Symbol, statements: usize, max: usize) {
    let noun = plural(statements, "statement", "statements");
    let message =
        format!("`{method}` callback has {statements} top-level {noun}, above the limit of {max}");
    span_lint_and_then(
        cx,
        MULTI_STATEMENT_ITERATOR_CLOSURE,
        callback.span,
        message,
        |diag| {
            diag.help(EXTRACTION_HELP);
            diag.help(MOVED_HELP);
            diag.note(REWRITE_NOTE);
        },
    );
}
