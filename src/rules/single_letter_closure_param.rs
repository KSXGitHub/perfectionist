//! `perfectionist::single_letter_closure_param` — flag closure
//! parameters whose identifier is one ASCII letter, unless the
//! closure is a trivial single-expression callback.
//!
//! The trivial-callback predicate lives in [`triviality`]; this file
//! owns the lint declaration, the configuration, and the late pass
//! that drives them.

use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use crate::common::{binding_ident, is_single_ascii_letter, merge_string_allowlist};

mod triviality;

use triviality::{is_trivial_wrapper, parent_call_callee_name, single_expression_body};

declare_tool_lint! {
    /// ### What it does
    /// Flags closure parameters whose identifier is one ASCII
    /// letter, unless the closure is a trivial single-expression
    /// callback. Two shapes qualify as trivial:
    /// - the closure is the immediate argument of a call whose
    ///   callee name is in the comparison / fold allowlist
    ///   (`sort_by`, `sort_by_key`, `min_by`, `max_by`,
    ///   `binary_search_by`, `cmp_by`, `partial_cmp_by`,
    ///   `fold`, `try_fold`, …);
    /// - the body is a trivial wrapper around the parameter —
    ///   a field access (`|x| x.field`), a method call
    ///   (`|x| x.foo()`), a one-argument call where the
    ///   parameter is the sole argument (`|x| vec![x]`), or a
    ///   reference (`|x| &x`). Surrounding `*` / `&` operators
    ///   around the parameter inside any of these shapes are
    ///   peeled before the match, so `|s| (*s).foo()` qualifies.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// A multi-line closure body whose parameter is a single
    /// letter forces the reader to scroll back to the closure
    /// header for context on every reference. The
    /// trivial-callback exception covers `sort_by(|a, b| ...)` and
    /// `.map(|x| x.field)` shapes that are short enough that the
    /// parameter's role is unambiguous from the call site.
    ///
    /// ### Example
    /// ```rust,ignore
    /// .map(|t| {
    ///     let columns = build_columns(t);
    ///     format_row(&columns)
    /// })
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// .map(|tree_row| {
    ///     let columns = build_columns(tree_row);
    ///     format_row(&columns)
    /// })
    /// ```
    pub perfectionist::SINGLE_LETTER_CLOSURE_PARAM,
    Warn,
    "closure parameter has a single-letter name",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::single_letter_closure_param";

/// Default allowlist of method names whose closure argument may
/// use single-letter parameters when the body is a single
/// expression. Both source documents agree on this list.
const DEFAULT_COMPARISON_METHODS: &[&str] = &[
    "sort_by",
    "sort_unstable_by",
    "sort_by_key",
    "sort_unstable_by_key",
    "min_by",
    "max_by",
    "min_by_key",
    "max_by_key",
    "binary_search_by",
    "binary_search_by_key",
    // `Iterator::cmp_by` / `partial_cmp_by` / `eq_by` take a
    // closure of two parameters. The bare `cmp` / `partial_cmp`
    // trait methods are not closure-accepting, so they are not
    // listed here.
    "cmp_by",
    "partial_cmp_by",
    "eq_by",
    "fold",
    "try_fold",
    "rfold",
    "reduce",
];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Additional method / function names whose closure argument
    /// may carry single-letter parameters when the body is a
    /// single expression. The entries listed here are merged with
    /// the built-in allowlist rather than replacing it, so a
    /// project only needs to enumerate its own DSL helpers
    /// (`when`, `iter_by`, third-party comparators such as
    /// `into_sorted_by`, …) and still benefits from the curated
    /// `core` / `std` defaults.
    extra_comparison_methods: Vec<String>,
    /// Method / function names to drop from the allowlist, even if
    /// they appear in the built-in defaults or in
    /// `extra_comparison_methods`. Useful for opting back into
    /// linting on a default entry the project does not consider
    /// trivial. Empty by default; checked after the merge with the
    /// built-ins, so this knob always wins.
    ignore_comparison_methods: Vec<String>,
}

pub struct SingleLetterClosureParam {
    comparison_methods: BTreeSet<String>,
}

impl SingleLetterClosureParam {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let comparison_methods = merge_string_allowlist(
            DEFAULT_COMPARISON_METHODS,
            config.extra_comparison_methods,
            config.ignore_comparison_methods,
        );
        Self { comparison_methods }
    }
}

impl_lint_pass!(SingleLetterClosureParam => [SINGLE_LETTER_CLOSURE_PARAM]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_CLOSURE_PARAM]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(SingleLetterClosureParam::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterClosureParam {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        let hir::ExprKind::Closure(closure) = expr.kind else {
            return;
        };
        let body = lint_context.tcx.hir_body(closure.body);
        let single_letter_params: Vec<rustc_span::Ident> = body
            .params
            .iter()
            .filter_map(|param| {
                let ident = binding_ident(param.pat)?;
                is_single_ascii_letter(ident.name.as_str()).then_some(ident)
            })
            .collect();
        if single_letter_params.is_empty() {
            return;
        }
        if self.closure_is_trivial(lint_context, expr, body) {
            return;
        }
        for ident in single_letter_params {
            span_lint_and_help(
                lint_context,
                SINGLE_LETTER_CLOSURE_PARAM,
                ident.span,
                format!(
                    "closure parameter `{}` has a single-letter name",
                    ident.name,
                ),
                None,
                "rename to a descriptive identifier, or rewrite the closure as \
                 a trivial single-expression callback",
            );
        }
    }
}

impl SingleLetterClosureParam {
    fn closure_is_trivial<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        closure_expr: &'tcx hir::Expr<'tcx>,
        body: &'tcx hir::Body<'tcx>,
    ) -> bool {
        let Some(body_expr) = single_expression_body(body) else {
            return false;
        };
        if self.is_in_comparison_call(lint_context, closure_expr) {
            return true;
        }
        if is_trivial_wrapper(body_expr, body.params) {
            return true;
        }
        false
    }

    fn is_in_comparison_call<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        closure_expr: &'tcx hir::Expr<'tcx>,
    ) -> bool {
        let Some(name) = parent_call_callee_name(lint_context, closure_expr) else {
            return false;
        };
        self.comparison_methods.contains(name.as_str())
    }
}
