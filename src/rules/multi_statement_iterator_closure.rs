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
    /// Flags inline callbacks to standard `Iterator` methods whose bodies
    /// contain more top-level statements than `max_statements` permits.
    /// The value the body produces, written without a semicolon, counts
    /// as a statement. A macro invocation counts as one statement, even
    /// when it expands to nothing. Comments, line wrapping, and empty
    /// semicolons do not count. A single expression stays one even when
    /// it contains a `match`, an `if`, or a nested block. A body behind
    /// parentheses or an `async`, `gen`, `try` or `const` wrapper is
    /// counted through, as an `unsafe` or labelled block is.
    ///
    /// The check covers transforming, filtering, visiting, generating and
    /// reducing callbacks, in method calls and qualified calls such as
    /// `Iterator::map`. A method that merely shares one of those names —
    /// a parallel iterator's `map`, an asynchronous stream's — is left
    /// alone: the rule asks the type system, not the spelling.
    ///
    /// A callback is measured only where the author's own source shows it
    /// passed to one of the covered methods. A call assembled by a macro
    /// is left alone, as is a callback inside a macro invocation's
    /// arguments.
    ///
    /// Projects differ on whether several steps belong inline or in a
    /// named helper, so the rule is inactive by default — enable it per
    /// crate by adding to `dylint.toml`:
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
    /// helper, or as a loop body when the operation is eager.
    ///
    /// Extracting a helper preserves a chain's structure. A loop rewrite
    /// needs more care: preserve laziness, evaluation order, ownership,
    /// allocation behaviour, and short-circuit results. A closure-local
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
    /// **Avoid:** 2 top-level statements — the `let`, then the tail
    /// expression
    ///
    /// ```rust,ignore
    /// entries.iter().map(|entry| {
    ///     let name = normalize(entry.name());
    ///     format!("{name}: {}", entry.value())
    /// })
    /// ```
    ///
    /// **Prefer:** a name for the operation, with the chain still lazy
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
        if callback.span.from_expansion() {
            return;
        }
        // The lookup is what establishes that the author wrote this
        // callback, and writes it to one of the methods the rule
        // covers: `count_source_statements` records a range only for a
        // closure the source shows in that position. So a call a macro
        // assembled around the author's closure finds nothing here,
        // and no question about where either span came from has to
        // stand in for reading the source.
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

/// How to tell the fix did not work, in the shape the sibling
/// threshold rules use: a test the reader applies to the result.
/// Naming the shapes this rule leaves alone instead — a closure bound
/// to a `let` and passed by name, and a body wrapped in one more block
/// — would print a working bypass under a `help:` label, where it
/// reads as a recipe rather than as a caution.
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
