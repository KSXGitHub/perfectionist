use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

use crate::common::{binding_ident, is_single_ascii_letter};

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

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Method / function names whose closure argument may carry
    /// single-letter parameters when the body is a single
    /// expression. Extend this list to add project-specific DSL
    /// helpers (`when`, `iter_by`, …).
    comparison_methods: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            comparison_methods: DEFAULT_COMPARISON_METHODS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

pub struct SingleLetterClosureParam {
    comparison_methods: BTreeSet<String>,
}

impl SingleLetterClosureParam {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            comparison_methods: config.comparison_methods.into_iter().collect(),
        }
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
        let parent = lint_context.tcx.parent_hir_node(closure_expr.hir_id);
        let hir::Node::Expr(parent_expr) = parent else {
            return false;
        };
        let callee_name = match parent_expr.kind {
            hir::ExprKind::MethodCall(segment, _, _, _) => Some(segment.ident.name),
            hir::ExprKind::Call(callee, _) => path_final_segment(callee),
            _ => None,
        };
        let Some(name) = callee_name else {
            return false;
        };
        self.comparison_methods.contains(name.as_str())
    }
}

fn binding_hir_id<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<hir::HirId> {
    match pat.kind {
        hir::PatKind::Binding(_, hir_id, _, None) => Some(hir_id),
        _ => None,
    }
}

/// If `body.value` is a single expression — either directly or
/// wrapped in a block with no statements — return that
/// expression. Otherwise return `None`.
fn single_expression_body<'hir>(body: &'hir hir::Body<'hir>) -> Option<&'hir hir::Expr<'hir>> {
    let value = body.value;
    if let hir::ExprKind::Block(block, _) = value.kind {
        if !block.stmts.is_empty() {
            return None;
        }
        block.expr
    } else {
        Some(value)
    }
}

fn path_final_segment<'hir>(expr: &'hir hir::Expr<'hir>) -> Option<Symbol> {
    let hir::ExprKind::Path(qpath) = &expr.kind else {
        return None;
    };
    let segment = match qpath {
        hir::QPath::Resolved(_, path) => path.segments.last()?,
        hir::QPath::TypeRelative(_, segment) => *segment,
    };
    Some(segment.ident.name)
}

/// Returns whether `expr` is a "trivial wrapper" around one of
/// the closure's parameters:
/// - a field access `param.field`,
/// - a method call `param.foo(args)`,
/// - a one-argument call `f(param)`,
/// - a reference `&param`.
///
/// In every shape, `*` / `&` operators around the reference to
/// the parameter are peeled by `is_param_ref` before matching,
/// so `|s| (*s).foo()` and `|s| f(&*s)` both qualify.
fn is_trivial_wrapper<'hir>(expr: &'hir hir::Expr<'hir>, params: &'hir [hir::Param<'hir>]) -> bool {
    /// "Refers to a parameter, possibly through one or more `*` /
    /// `&` operators." Peeling through these keeps `|s| (*s).foo()`
    /// classified as a trivial wrapper, since the deref is a
    /// purely-structural step the reader does not need help with.
    fn is_param_ref(expr: &hir::Expr<'_>, params: &[hir::Param<'_>]) -> bool {
        let mut expr = expr;
        loop {
            match &expr.kind {
                hir::ExprKind::Unary(hir::UnOp::Deref, inner)
                | hir::ExprKind::AddrOf(_, _, inner) => expr = inner,
                hir::ExprKind::Path(hir::QPath::Resolved(None, path)) => {
                    let Res::Local(local_hir_id) = path.res else {
                        return false;
                    };
                    return params
                        .iter()
                        .any(|param| binding_hir_id(param.pat) == Some(local_hir_id));
                }
                _ => return false,
            }
        }
    }
    match expr.kind {
        hir::ExprKind::Field(receiver, _) => is_param_ref(receiver, params),
        hir::ExprKind::MethodCall(_, receiver, _, _) => is_param_ref(receiver, params),
        hir::ExprKind::Call(_, args) => args.len() == 1 && is_param_ref(&args[0], params),
        hir::ExprKind::AddrOf(_, _, inner) => is_param_ref(inner, params),
        _ => false,
    }
}
