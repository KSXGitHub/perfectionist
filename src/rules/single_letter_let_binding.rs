use crate::ascii_letter::AsciiLetter;
use crate::common::{
    DefaultState, binding_ident, hir_in_external_macro, is_single_ascii_letter,
    resolve_symbol_set_from_chars,
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
    /// Flags `let x = ...;` bindings whose identifier is one ASCII
    /// letter.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// A descriptive `let` binding documents what the right-hand
    /// side computed; a single-letter name does not. The rule
    /// allows `let n = ...` and other names in a configurable
    /// set of exempt identifiers for the well-worn cases
    /// (unsigned counts).
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::min_ident_chars` (`restriction`, off by default)
    /// also flags single-character `let` bindings, under a single
    /// crate-wide `min-ident-chars-threshold` and one shared
    /// allow-list spanning several kinds at once (bindings,
    /// parameters, item names). This rule governs `let` bindings
    /// alone, with its own exempt set. Enabling both is redundant
    /// for `let` bindings — choose per-kind control here, or
    /// `min_ident_chars` for the one-knob sweep.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let m = entry.metadata()?;
    /// ```
    ///
    /// **Prefer:**
    ///
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
    /// standard ones. Each entry is a single ASCII letter
    /// (`a`-`z`, `A`-`Z`); any other character is rejected at
    /// config-parse time.
    extra_allowed_idents: Vec<AsciiLetter>,
    /// Identifiers to deny (always flag), removing them from the
    /// exempt set even if they appear in the built-in defaults or
    /// in `extra_allowed_idents`. Empty by default; checked after
    /// the merge with the built-ins, so this knob always wins.
    /// Each entry is a single ASCII letter (`a`-`z`, `A`-`Z`);
    /// any other character is rejected at config-parse time.
    extra_denied_idents: Vec<AsciiLetter>,
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
            config.extra_denied_idents,
        );
        Self { allowed_idents }
    }
}

impl_lint_pass!(SingleLetterLetBinding => [SINGLE_LETTER_LET_BINDING]);

impl Register for rule::SingleLetterLetBinding {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SINGLE_LETTER_LET_BINDING]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(SingleLetterLetBinding::new())));
    }
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
