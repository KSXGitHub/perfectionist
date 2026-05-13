use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use clippy_utils::diagnostics::span_lint_and_help;

use crate::common::is_single_ascii_letter;

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

const CONFIG_KEY: &str = "perfectionist::single_letter_generic";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Maximum number of source lines an `impl Trait for Type`
    /// block may span and still permit single-letter generic
    /// parameter names. Defaults to `20`.
    short_impl_max_lines: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            short_impl_max_lines: 20,
        }
    }
}

pub struct SingleLetterGeneric {
    short_impl_max_lines: usize,
}

impl SingleLetterGeneric {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            short_impl_max_lines: config.short_impl_max_lines,
        }
    }
}

impl_lint_pass!(SingleLetterGeneric => [SINGLE_LETTER_GENERIC]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_GENERIC]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if !crate::common::is_enabled("single_letter_generic", true) {
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
}

impl SingleLetterGeneric {
    fn enclosing_short_trait_impl<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        param_hir_id: hir::HirId,
    ) -> bool {
        for (_, node) in lint_context.tcx.hir_parent_iter(param_hir_id) {
            match node {
                hir::Node::Item(item) => {
                    let hir::ItemKind::Impl(impl_block) = &item.kind else {
                        return false;
                    };
                    if impl_block.of_trait.is_none() {
                        return false;
                    }
                    return span_line_count(lint_context, item.span) <= self.short_impl_max_lines;
                }
                // A generic on a method (or other associated item) inside
                // an impl is owned by the method, not by the impl. The
                // short-trait-impl exemption applies only to generics
                // owned by the impl block itself, so stop here rather
                // than walk through to the surrounding impl.
                hir::Node::ImplItem(_) | hir::Node::TraitItem(_) | hir::Node::ForeignItem(_) => {
                    return false;
                }
                _ => continue,
            }
        }
        false
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
