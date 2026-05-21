use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;

use crate::common::{
    DefaultState, binding_ident, deserialize_ascii_letters, hir_in_external_macro,
    is_single_ascii_letter, resolve_symbol_set_from_chars, resolved_state,
};

declare_tool_lint! {
    /// ### What it does
    /// Flags `let x = ...;` bindings whose identifier is one ASCII
    /// letter.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// A descriptive `let` binding documents what the right-hand
    /// side computed; a single-letter name does not. The rule
    /// allows `let n = ...` and other names in a configurable
    /// set of exempt identifiers for the well-worn cases
    /// (unsigned counts).
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

/// Default exempt identifiers for `let` bindings. A short
/// unsigned count (`n`) is the most common idiom.
const DEFAULT_LET_EXEMPTIONS: &[char] = &['n'];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Additional identifiers to allow as `let` binding names.
    /// Merged with the built-in defaults (`["n"]`); empty by
    /// default. Use this to whitelist project-specific
    /// conventional names without having to re-state the
    /// standard ones.
    #[serde(deserialize_with = "deserialize_ascii_letters")]
    extra_allowed_idents: Vec<char>,
    /// Identifiers to drop from the exempt set, even if they
    /// appear in the built-in defaults or in
    /// `extra_allowed_idents`. Empty by default; checked after
    /// the merge with the built-ins, so this knob always wins.
    #[serde(deserialize_with = "deserialize_ascii_letters")]
    ignore_allowed_idents: Vec<char>,
}

pub struct SingleLetterLetBinding {
    allowed_idents: BTreeSet<Symbol>,
}

impl SingleLetterLetBinding {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let allowed_idents = resolve_symbol_set_from_chars(
            DEFAULT_LET_EXEMPTIONS,
            config.extra_allowed_idents,
            config.ignore_allowed_idents,
        );
        Self { allowed_idents }
    }
}

impl_lint_pass!(SingleLetterLetBinding => [SINGLE_LETTER_LET_BINDING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_LET_BINDING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("single_letter_let_binding", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(SingleLetterLetBinding::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterLetBinding {
    fn check_local(&mut self, lint_context: &LateContext<'tcx>, local: &'tcx hir::LetStmt<'tcx>) {
        if !matches!(local.source, hir::LocalSource::Normal) {
            // `for` / `while let` desugarings synthesise `LetStmt`
            // nodes with names the user did not write.
            return;
        }
        if hir_in_external_macro(lint_context, local.hir_id, local.span) {
            return;
        }
        let Some(ident) = binding_ident(local.pat) else {
            return;
        };
        if !is_single_ascii_letter(ident.name.as_str()) {
            return;
        }
        if self.allowed_idents.contains(&ident.name) {
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
