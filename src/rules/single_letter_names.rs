use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_in_test;
use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};

declare_tool_lint! {
    /// ### What it does
    /// Flags generic type parameters whose identifier is one ASCII
    /// letter (`T`, `U`, `K`, `V`, …), except inside trait `impl`
    /// blocks whose body fits within a small line threshold.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// Single-letter generic names propagate through the type
    /// signatures and bounds; in a long impl block they force
    /// every reader to scroll back to the `impl` header to recover
    /// the role of each parameter. Descriptive names
    /// (`Element`, `Key`, `Reader`) keep complex signatures
    /// self-documenting. The short-trait-impl exception covers
    /// the canonical `impl<T> From<T> for Wrapper<T>` shape
    /// where the body is small enough that a reader cannot lose
    /// track of `T`.
    ///
    /// ### Example
    /// ```rust,ignore
    /// pub fn collect_keys<K, V>(map: BTreeMap<K, V>) -> Vec<K> {
    ///     /* fifty lines */
    /// }
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// pub fn collect_keys<Key, Value>(map: BTreeMap<Key, Value>) -> Vec<Key> {
    ///     /* fifty lines */
    /// }
    /// ```
    pub perfectionist::SINGLE_LETTER_GENERIC,
    Warn,
    "generic type parameter has a single-letter name",
    report_in_external_macro: false
}

declare_tool_lint! {
    /// ### What it does
    /// Flags `let x = ...;` bindings whose identifier is one ASCII
    /// letter, outside `#[cfg(test)]` code.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// A descriptive `let` binding documents what the right-hand
    /// side computed; a single-letter name does not. The rule
    /// allows `let n = ...` and other names in a configurable
    /// allowlist for the well-worn cases (unsigned counts), and
    /// switches off entirely under `#[cfg(test)]` where fixtures
    /// such as `let a = ...; let b = ...;` for interchangeable
    /// specimens are a recognised idiom.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let m = entry.metadata()?;
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let metadata = entry.metadata()?;
    /// ```
    pub perfectionist::SINGLE_LETTER_LET_BINDING,
    Warn,
    "`let` binding has a single-letter name",
    report_in_external_macro: false
}

declare_tool_lint! {
    /// ### What it does
    /// Flags function and method parameters whose identifier is
    /// one ASCII letter, except for a curated set of conventional
    /// names (`n` for an unsigned count, `f` for a `fmt::Formatter`,
    /// `i` / `j` / `k` for indices).
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// Parameter names are the first piece of documentation a
    /// caller reads (in rustdoc, in IDE hover tips, in error
    /// messages). A descriptive parameter name carries that
    /// documentation; a single letter does not.
    ///
    /// ### Example
    /// ```rust,ignore
    /// fn write_row(w: &mut Writer, t: &TreeRow) -> io::Result<()> { ... }
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// fn write_row(writer: &mut Writer, tree_row: &TreeRow) -> io::Result<()> { ... }
    /// ```
    pub perfectionist::SINGLE_LETTER_FUNCTION_PARAM,
    Warn,
    "function parameter has a single-letter name",
    report_in_external_macro: false
}

declare_tool_lint! {
    /// ### What it does
    /// Flags closure parameters whose identifier is one ASCII
    /// letter, unless the closure is a trivial single-expression
    /// callback. Two shapes qualify as trivial:
    /// - the closure is the immediate argument of a call whose
    ///   callee name is in the comparison / fold allowlist
    ///   (`sort_by`, `cmp`, `partial_cmp`, `min_by`, `max_by`,
    ///   `binary_search_by`, `fold`, `try_fold`, …);
    /// - the body is a trivial wrapper around the parameter —
    ///   a field access (`|x| x.field`), a method call
    ///   (`|x| x.foo()`), or a one-argument call where the
    ///   parameter is the sole argument (`|x| vec![x]`).
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// A multi-line closure body whose parameter is a single
    /// letter forces the reader to scroll back to the closure
    /// header for context on every reference. The trivial-
    /// callback exception covers `sort_by(|a, b| ...)` and
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

const CONFIG_KEY: &str = "perfectionist::single_letter_names";

/// Default allowlist for `let` bindings, applied on top of the
/// `#[cfg(test)]` exemption. A short unsigned count (`n`) is the
/// most common idiom that survives outside test code.
const DEFAULT_LET_ALLOWLIST: &[&str] = &["n"];

/// Default allowlist for function and method parameters: the
/// canonical names from both source documents (`n` for an
/// unsigned count, `f` for `fmt::Formatter`, `i` / `j` / `k` for
/// indices).
const DEFAULT_FN_PARAM_ALLOWLIST: &[&str] = &["n", "f", "i", "j", "k"];

/// Default allowlist of method names whose closure argument may
/// use single-letter parameters when the body is a single
/// expression. Both source documents agree on this list.
const DEFAULT_COMPARISON_METHODS: &[&str] = &[
    "sort_by",
    "sort_unstable_by",
    "sort_by_key",
    "sort_unstable_by_key",
    "cmp",
    "partial_cmp",
    "min_by",
    "max_by",
    "min_by_key",
    "max_by_key",
    "binary_search_by",
    "binary_search_by_key",
    "fold",
    "try_fold",
    "rfold",
    "reduce",
];

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Identifiers that are always allowed as `let` binding
    /// names, even outside `#[cfg(test)]` code. Defaults to
    /// `["n"]`.
    let_binding_allowed_idents: Vec<String>,
    /// Identifiers that are always allowed as function or method
    /// parameter names. Defaults to `["n", "f", "i", "j", "k"]`.
    fn_param_allowed_idents: Vec<String>,
    /// Maximum number of source lines an `impl Trait for Type`
    /// block may span and still permit single-letter generic
    /// parameter names. Defaults to `20`.
    short_impl_max_lines: usize,
    /// Method / function names whose closure argument may carry
    /// single-letter parameters when the body is a single
    /// expression. Extend this list to add project-specific DSL
    /// helpers (`when`, `iter_by`, …).
    comparison_methods: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            let_binding_allowed_idents: DEFAULT_LET_ALLOWLIST
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            fn_param_allowed_idents: DEFAULT_FN_PARAM_ALLOWLIST
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            short_impl_max_lines: 20,
            comparison_methods: DEFAULT_COMPARISON_METHODS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

pub struct SingleLetterNames {
    let_binding_allowed_idents: BTreeSet<String>,
    fn_param_allowed_idents: BTreeSet<String>,
    short_impl_max_lines: usize,
    comparison_methods: BTreeSet<String>,
}

impl SingleLetterNames {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            let_binding_allowed_idents: config.let_binding_allowed_idents.into_iter().collect(),
            fn_param_allowed_idents: config.fn_param_allowed_idents.into_iter().collect(),
            short_impl_max_lines: config.short_impl_max_lines,
            comparison_methods: config.comparison_methods.into_iter().collect(),
        }
    }
}

impl_lint_pass!(SingleLetterNames => [
    SINGLE_LETTER_GENERIC,
    SINGLE_LETTER_LET_BINDING,
    SINGLE_LETTER_FUNCTION_PARAM,
    SINGLE_LETTER_CLOSURE_PARAM,
]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[
        SINGLE_LETTER_GENERIC,
        SINGLE_LETTER_LET_BINDING,
        SINGLE_LETTER_FUNCTION_PARAM,
        SINGLE_LETTER_CLOSURE_PARAM,
    ]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(SingleLetterNames::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterNames {
    fn check_generic_param(
        &mut self,
        lint_context: &LateContext<'tcx>,
        param: &'tcx hir::GenericParam<'tcx>,
    ) {
        let hir::GenericParamKind::Type { synthetic, .. } = param.kind else {
            return;
        };
        if synthetic {
            // `impl Trait`-desugared parameters carry compiler-generated
            // names that the user did not write; the rule does not apply.
            return;
        }
        let ident = param.name.ident();
        if !is_single_ascii_letter(ident.name.as_str()) {
            return;
        }
        if self.enclosing_short_trait_impl(lint_context, param.hir_id) {
            return;
        }
        span_lint_and_help(
            lint_context,
            SINGLE_LETTER_GENERIC,
            param.span,
            format!(
                "generic type parameter `{}` has a single-letter name",
                ident.name,
            ),
            None,
            "rename to a descriptive identifier (e.g. `Element`, `Key`, `Reader`)",
        );
    }

    fn check_local(&mut self, lint_context: &LateContext<'tcx>, local: &'tcx hir::LetStmt<'tcx>) {
        if !matches!(local.source, hir::LocalSource::Normal) {
            // `for` / `while let` desugarings synthesise `LetStmt`
            // nodes with names the user did not write.
            return;
        }
        let Some(ident) = binding_ident(local.pat) else {
            return;
        };
        if !is_single_ascii_letter(ident.name.as_str()) {
            return;
        }
        if self
            .let_binding_allowed_idents
            .contains(ident.name.as_str())
        {
            return;
        }
        if is_in_test(lint_context.tcx, local.hir_id) {
            return;
        }
        span_lint_and_help(
            lint_context,
            SINGLE_LETTER_LET_BINDING,
            ident.span,
            format!("`let` binding `{}` has a single-letter name", ident.name),
            None,
            "rename to a descriptive identifier",
        );
    }

    fn check_fn(
        &mut self,
        lint_context: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        _def_id: rustc_span::def_id::LocalDefId,
    ) {
        match kind {
            FnKind::ItemFn(..) | FnKind::Method(..) => {
                self.check_function_params(lint_context, decl, body);
            }
            FnKind::Closure => {
                // Closure parameters are handled via `check_expr` so
                // the parent call context is available.
            }
        }
    }

    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        let hir::ExprKind::Closure(closure) = expr.kind else {
            return;
        };
        self.check_closure_params(lint_context, expr, closure);
    }
}

impl SingleLetterNames {
    fn check_function_params<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
    ) {
        // The first param of a method is `self`, whose pattern is the
        // implicit-self synthesised binding; the rule does not flag it.
        let skip_self = !matches!(decl.implicit_self, hir::ImplicitSelfKind::None);
        let params_iter = body.params.iter().skip(usize::from(skip_self));
        for param in params_iter {
            let Some(ident) = binding_ident(param.pat) else {
                continue;
            };
            if !is_single_ascii_letter(ident.name.as_str()) {
                continue;
            }
            if self.fn_param_allowed_idents.contains(ident.name.as_str()) {
                continue;
            }
            span_lint_and_help(
                lint_context,
                SINGLE_LETTER_FUNCTION_PARAM,
                ident.span,
                format!(
                    "function parameter `{}` has a single-letter name",
                    ident.name,
                ),
                None,
                "rename to a descriptive identifier",
            );
        }
    }

    fn check_closure_params<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        closure_expr: &'tcx hir::Expr<'tcx>,
        closure: &'tcx hir::Closure<'tcx>,
    ) {
        let body = lint_context.tcx.hir_body(closure.body);
        let single_letter_params: Vec<(rustc_span::Ident, hir::HirId)> = body
            .params
            .iter()
            .filter_map(|param| {
                let ident = binding_ident(param.pat)?;
                if is_single_ascii_letter(ident.name.as_str()) {
                    let binding_hir_id = binding_hir_id(param.pat)?;
                    Some((ident, binding_hir_id))
                } else {
                    None
                }
            })
            .collect();
        if single_letter_params.is_empty() {
            return;
        }
        let is_trivial = self.closure_is_trivial(lint_context, closure_expr, body);
        if is_trivial {
            return;
        }
        for (ident, _) in single_letter_params {
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

    fn enclosing_short_trait_impl<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        param_hir_id: hir::HirId,
    ) -> bool {
        for (_, node) in lint_context.tcx.hir_parent_iter(param_hir_id) {
            let hir::Node::Item(item) = node else {
                continue;
            };
            let hir::ItemKind::Impl(impl_block) = &item.kind else {
                return false;
            };
            if impl_block.of_trait.is_none() {
                return false;
            }
            return span_line_count(lint_context, item.span) <= self.short_impl_max_lines;
        }
        false
    }
}

fn is_single_ascii_letter(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    chars.next().is_none() && first.is_ascii_alphabetic()
}

fn binding_ident<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<rustc_span::Ident> {
    match pat.kind {
        hir::PatKind::Binding(_, _, ident, None) => Some(ident),
        _ => None,
    }
}

fn binding_hir_id<'hir>(pat: &'hir hir::Pat<'hir>) -> Option<hir::HirId> {
    match pat.kind {
        hir::PatKind::Binding(_, hir_id, _, None) => Some(hir_id),
        _ => None,
    }
}

fn span_line_count(lint_context: &LateContext<'_>, span: Span) -> usize {
    let source_map = lint_context.sess().source_map();
    let (_, lo_line, _, hi_line, _) = source_map.span_to_location_info(span);
    if lo_line == 0 || hi_line < lo_line {
        return usize::MAX;
    }
    hi_line - lo_line + 1
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
