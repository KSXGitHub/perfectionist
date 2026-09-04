use crate::ascii_letter::AsciiLetter;
use crate::common::{
    DefaultState, hir_in_external_macro, is_single_ascii_letter, resolve_symbol_set_from_chars,
};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;
use std::collections::BTreeSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags const generic parameter declarations
    /// (`<const N: usize>`) whose identifier is one ASCII letter.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// A single-letter const generic parameter is opaque at every
    /// use site; a descriptive identifier (`LEN`, `COLS`, `LANES`)
    /// documents the parameter's role both at the declaration and
    /// at every substitution.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::min_ident_chars` (`restriction`, off by default)
    /// flags short identifiers in many positions, but **explicitly
    /// skips const generic parameters** — its visitor returns early
    /// on `GenericParamKind::Const`, so it never flags
    /// `<const N: usize>`. This rule covers exactly that gap, so the
    /// two are complementary rather than redundant: enable both to
    /// also catch short bindings, parameters, and item names.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// struct Data<Left, Right, const M: usize, const N: usize> {
    ///     left:  [Left;  M],
    ///     right: [Right; N],
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// struct Data<Left, Right, const LEFT_LEN: usize, const RIGHT_LEN: usize> {
    ///     left:  [Left;  LEFT_LEN],
    ///     right: [Right; RIGHT_LEN],
    /// }
    /// ```
    pub perfectionist::SINGLE_LETTER_CONST_GENERIC,
    Warn,
    "const generic parameter has a single-letter name",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::single_letter_const_generic";

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Identifiers the rule will not flag. Empty by default. Each
    /// entry is a single ASCII letter (`a`-`z`, `A`-`Z`); any other
    /// character is rejected at config-parse time.
    allowed_idents: Vec<AsciiLetter>,
}

pub struct SingleLetterConstGeneric {
    allowed_idents: BTreeSet<Symbol>,
}

impl SingleLetterConstGeneric {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let allowed_idents =
            resolve_symbol_set_from_chars(&[], config.allowed_idents, Vec::<AsciiLetter>::new());
        Self { allowed_idents }
    }
}

impl_lint_pass!(SingleLetterConstGeneric => [SINGLE_LETTER_CONST_GENERIC]);

impl Register for rule::SingleLetterConstGeneric {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SINGLE_LETTER_CONST_GENERIC]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_pass(|_| Box::new(SingleLetterConstGeneric::new()));
    }
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterConstGeneric {
    fn check_generic_param(
        &mut self,
        lint_context: &LateContext<'tcx>,
        param: &'tcx hir::GenericParam<'tcx>,
    ) {
        if !matches!(param.kind, hir::GenericParamKind::Const { .. }) {
            return;
        }
        if hir_in_external_macro(lint_context, param.hir_id, param.span) {
            return;
        }
        let ident = param.name.ident();
        if !is_single_ascii_letter(ident.name.as_str()) {
            return;
        }
        if self.allowed_idents.contains(&ident.name) {
            return;
        }
        span_lint_and_help(
            lint_context,
            SINGLE_LETTER_CONST_GENERIC,
            param.span,
            format!(
                "const generic parameter `{}` has a single-letter name",
                ident.name,
            ),
            None,
            "rename to a descriptive identifier (e.g. `LEN`, `COLS`, `LANES`)",
        );
    }
}
