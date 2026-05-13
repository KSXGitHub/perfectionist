use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_in_test;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use crate::common::{binding_ident, is_single_ascii_letter};

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

const CONFIG_KEY: &str = "perfectionist::single_letter_let_binding";

/// Default allowlist for `let` bindings, applied on top of the
/// `#[cfg(test)]` exemption. A short unsigned count (`n`) is the
/// most common idiom that survives outside test code.
const DEFAULT_LET_ALLOWLIST: &[&str] = &["n"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Identifiers that are always allowed as `let` binding
    /// names, even outside `#[cfg(test)]` code. Defaults to
    /// `["n"]`.
    allowed_idents: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allowed_idents: DEFAULT_LET_ALLOWLIST
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

pub struct SingleLetterLetBinding {
    allowed_idents: BTreeSet<String>,
}

impl SingleLetterLetBinding {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            allowed_idents: config.allowed_idents.into_iter().collect(),
        }
    }
}

impl_lint_pass!(SingleLetterLetBinding => [SINGLE_LETTER_LET_BINDING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_LET_BINDING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(SingleLetterLetBinding::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterLetBinding {
    fn check_local(&mut self, lint_context: &LateContext<'tcx>, local: &'tcx hir::LetStmt<'tcx>) {
        if !matches!(local.source, hir::LocalSource::Normal) {
            // `for` / `while let` desugarings synthesise `LetStmt`
            // nodes with names the user did not write.
            return;
        }
        if local
            .span
            .in_external_macro(lint_context.sess().source_map())
        {
            // Proc-macros such as `clap_derive`'s `default_value_t`
            // synthesise `let <one-letter> = ...;` bindings while
            // attaching a user-source span to the identifier. The
            // tool-lint's `report_in_external_macro: false` flag
            // inspects the diagnostic span (the identifier), which
            // looks like user code; the surrounding statement span
            // still carries the external expansion context, so the
            // explicit check is needed to suppress the lint.
            return;
        }
        let Some(ident) = binding_ident(local.pat) else {
            return;
        };
        if !is_single_ascii_letter(ident.name.as_str()) {
            return;
        }
        if self.allowed_idents.contains(ident.name.as_str()) {
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
}
