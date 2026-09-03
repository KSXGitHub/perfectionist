use crate::ascii_letter::AsciiLetter;
use crate::common::{
    DefaultState, hir_in_external_macro, is_single_ascii_letter, resolve_symbol_set_from_chars,
    resolved_state,
};
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Ident, Span, Symbol};
use std::collections::BTreeSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags `const` items (free, associated, and block-level)
    /// whose identifier is one ASCII letter.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// A single-letter `const` item is opaque at every use site,
    /// and the item's scope (module-wide or crate-wide for
    /// `pub const`) makes that opacity propagate. A descriptive
    /// identifier (`DIMENSION`, `BUFFER_LEN`, `MAX_RETRIES`)
    /// carries its own documentation.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::min_ident_chars` (`restriction`, off by default)
    /// also flags single-character `const` item names, under a
    /// single crate-wide `min-ident-chars-threshold` and one shared
    /// allow-list that spans several kinds at once — bindings,
    /// parameters, and item names. (It skips generic and
    /// const-generic parameters, which the `single_letter_*` family
    /// handles separately.) This rule governs `const` items alone,
    /// with its own `allowed_idents`. Enabling both is redundant for
    /// `const` items — choose per-kind control here, or
    /// `min_ident_chars` for the one-knob sweep.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// const N: usize = 2;
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// const DIMENSION_COUNT: usize = 2;
    /// ```
    pub perfectionist::SINGLE_LETTER_CONST_ITEM,
    Warn,
    "const item has a single-letter name",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::single_letter_const_item";

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Identifiers the rule will not flag. Empty by default. Each
    /// entry is a single ASCII letter (`a`-`z`, `A`-`Z`); any other
    /// character is rejected at config-parse time.
    allowed_idents: Vec<AsciiLetter>,
}

pub struct SingleLetterConstItem {
    allowed_idents: BTreeSet<Symbol>,
}

impl SingleLetterConstItem {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let allowed_idents =
            resolve_symbol_set_from_chars(&[], config.allowed_idents, Vec::<AsciiLetter>::new());
        Self { allowed_idents }
    }

    fn check(&self, cx: &LateContext<'_>, hir_id: hir::HirId, span: Span, ident: Ident) {
        if hir_in_external_macro(cx, hir_id, span) {
            return;
        }
        if !is_single_ascii_letter(ident.name.as_str()) {
            return;
        }
        if self.allowed_idents.contains(&ident.name) {
            return;
        }
        span_lint_and_help(
            cx,
            SINGLE_LETTER_CONST_ITEM,
            ident.span,
            format!("const item `{}` has a single-letter name", ident.name),
            None,
            "rename to a descriptive identifier (e.g. `DIMENSION`, `BUFFER_LEN`, `MAX_RETRIES`)",
        );
    }
}

impl_lint_pass!(SingleLetterConstItem => [SINGLE_LETTER_CONST_ITEM]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_CONST_ITEM]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("single_letter_const_item", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_lint_pass(Box::new(|_| Box::new(SingleLetterConstItem::new())));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterConstItem {
    fn check_item(&mut self, lint_context: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let hir::ItemKind::Const(ident, _, _, _) = item.kind else {
            return;
        };
        self.check(lint_context, item.hir_id(), item.span, ident);
    }

    fn check_impl_item(
        &mut self,
        lint_context: &LateContext<'tcx>,
        item: &'tcx hir::ImplItem<'tcx>,
    ) {
        if !matches!(item.kind, hir::ImplItemKind::Const(..)) {
            return;
        }
        self.check(lint_context, item.hir_id(), item.span, item.ident);
    }

    fn check_trait_item(
        &mut self,
        lint_context: &LateContext<'tcx>,
        item: &'tcx hir::TraitItem<'tcx>,
    ) {
        if !matches!(item.kind, hir::TraitItemKind::Const(..)) {
            return;
        }
        self.check(lint_context, item.hir_id(), item.span, item.ident);
    }
}
