use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::rules::common::{binding_ident, is_single_ascii_letter};

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

const CONFIG_KEY: &str = "perfectionist::single_letter_function_param";

/// Default allowlist for function and method parameters: the
/// canonical names from both source documents (`n` for an
/// unsigned count, `f` for `fmt::Formatter`, `i` / `j` / `k` for
/// indices).
const DEFAULT_FN_PARAM_ALLOWLIST: &[&str] = &["n", "f", "i", "j", "k"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Identifiers that are always allowed as function or method
    /// parameter names. Defaults to `["n", "f", "i", "j", "k"]`.
    fn_param_allowed_idents: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fn_param_allowed_idents: DEFAULT_FN_PARAM_ALLOWLIST
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

pub struct SingleLetterFunctionParam {
    fn_param_allowed_idents: BTreeSet<String>,
}

impl SingleLetterFunctionParam {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            fn_param_allowed_idents: config.fn_param_allowed_idents.into_iter().collect(),
        }
    }
}

impl_lint_pass!(SingleLetterFunctionParam => [SINGLE_LETTER_FUNCTION_PARAM]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_FUNCTION_PARAM]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(SingleLetterFunctionParam::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterFunctionParam {
    fn check_fn(
        &mut self,
        lint_context: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        _def_id: rustc_span::def_id::LocalDefId,
    ) {
        if !matches!(kind, FnKind::ItemFn(..) | FnKind::Method(..)) {
            // Closure parameters are the closure rule's territory.
            return;
        }
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
}
