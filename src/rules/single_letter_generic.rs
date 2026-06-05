use crate::common::{DefaultState, hir_in_external_macro, is_single_ascii_letter, resolved_state};
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags generic type parameters whose identifier is one ASCII
    /// letter (`T`, `U`, `K`, `V`, ...).
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// Single-letter generic names propagate through the type
    /// signatures and bounds; they force every reader to scroll
    /// back to the declaration to recover the role of each
    /// parameter. Descriptive names (`Element`, `Key`, `Reader`)
    /// keep complex signatures self-documenting.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::min_ident_chars` (`restriction`, off by default)
    /// covers the same identifiers, but as a single crate-wide
    /// threshold and one shared allow-list spanning every kind at
    /// once — bindings, parameters, generics, and item names
    /// alike. This rule splits that blanket per kind: it targets
    /// generic type parameters only, leaving every other
    /// identifier kind to its own rule. Enabling both is
    /// redundant — choose per-kind control here, or
    /// `min_ident_chars` for the one-knob sweep.
    ///
    /// Single-character *lifetime* names (`'a`) are out of scope for
    /// the `single_letter_*` family; `clippy::single_char_lifetime_names`
    /// (`restriction`) covers those.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// pub fn collect_keys<K, V>(map: BTreeMap<K, V>) -> Vec<K> {
    ///     /* fifty lines */
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
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

const CONFIG_KEY: &str = "perfectionist::single_letter_generic";

/// Configuration is reserved for future knobs; the lint currently
/// has no options. The empty struct still exists so that a stray
/// `[perfectionist::single_letter_generic]` table in `dylint.toml`
/// deserialises rather than producing a confusing parse error.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct SingleLetterGeneric;

impl SingleLetterGeneric {
    fn new() -> Self {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self
    }
}

impl_lint_pass!(SingleLetterGeneric => [SINGLE_LETTER_GENERIC]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_GENERIC]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("single_letter_generic", DefaultState::Active) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(SingleLetterGeneric::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterGeneric {
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
        if hir_in_external_macro(lint_context, param.hir_id, param.span) {
            return;
        }
        let ident = param.name.ident();
        if !is_single_ascii_letter(ident.name.as_str()) {
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
}
