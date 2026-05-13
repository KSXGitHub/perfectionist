use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::common::{binding_ident, is_single_ascii_letter, merge_string_allowlist};

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

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Additional identifiers to allow as function or method
    /// parameter names. Merged with the built-in defaults
    /// (`["n", "f", "i", "j", "k"]`); empty by default. Use this
    /// to whitelist project-specific conventional names without
    /// having to re-state the standard ones.
    extra_allowed_idents: Vec<String>,
    /// Identifiers to drop from the allowlist, even if they appear
    /// in the built-in defaults or in `extra_allowed_idents`.
    /// Empty by default; checked after the merge with the
    /// built-ins, so this knob always wins.
    ignore_allowed_idents: Vec<String>,
}

pub struct SingleLetterFunctionParam {
    allowed_idents: BTreeSet<String>,
}

impl SingleLetterFunctionParam {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let allowed_idents = merge_string_allowlist(
            DEFAULT_FN_PARAM_ALLOWLIST,
            config.extra_allowed_idents,
            config.ignore_allowed_idents,
        );
        Self { allowed_idents }
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
            if self.allowed_idents.contains(ident.name.as_str()) {
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
