use crate::common::{DefaultState, resolved_state};
use rustc_ast::Attribute;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol, sym};

mod emit;
mod insertion;
mod scan;

declare_tool_lint! {
    /// ### What it does
    ///
    /// When a lint-level attribute (`#[allow]`, `#[expect]`, `#[warn]`,
    /// `#[deny]`, `#[forbid]`) carries a trailing `// ...` line comment
    /// — on the same source line as the attribute's closing `]` —
    /// that documents *why* the level was chosen, lifts the comment
    /// into the attribute's `reason = "..."` field and removes the
    /// original comment.
    ///
    /// Only the trailing placement counts: a same-line comment after
    /// `]` is unambiguously about the attribute. A comment on the
    /// *preceding* line is intentionally out of scope — it is just as
    /// often documentation for the next item as it is attribute
    /// rationale, and a static check cannot tell the two apart.
    ///
    /// Doc comments (`///`, `//!`) and block comments (`/* ... */`)
    /// are out of scope.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// `reason = "..."` is part of the attribute and travels with it
    /// through every refactor; a free-floating comment can be
    /// separated from its attribute by an unrelated edit. Compiler
    /// diagnostics render the `reason` field in the lint's message,
    /// so the rationale reaches the reader at the moment of confusion.
    /// One canonical location for the rationale also removes the
    /// "is this comment for the attribute, or for the next item?"
    /// question.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments)] // matches upstream signature
    /// fn build_fetcher(/* ... */) {}
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    pub perfectionist::LINT_ATTRIBUTE_TRAILING_COMMENT,
    Warn,
    r#"trailing comment on a lint-level attribute should be lifted into a `reason = "..."` field"#,
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::lint_attribute_trailing_comment";

/// The rule has no configuration knobs. The empty struct is
/// load-bearing rather than a placeholder: it makes a mistyped key
/// under the rule's `dylint.toml` table an error instead of a
/// silent no-op, and renders as `Configuration: none.` in the
/// catalogue. See "Rules with no configuration" in
/// `planned-rules/IMPLEMENTATION_CONVENTIONS.md`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct LintAttributeTrailingComment {
    /// Outer span of the enclosing `#[cfg_attr(...)]`, carried from
    /// its `cfg_attr_trace` to the synthesised inner lint-level
    /// attribute so the latter anchors its comment search to the
    /// original source rather than its own narrow span. See
    /// [`Self::check_attribute`] for the mechanism.
    pending_cfg_attr_outer: Option<Span>,
}

impl LintAttributeTrailingComment {
    fn new() -> Self {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            pending_cfg_attr_outer: None,
        }
    }
}

impl_lint_pass!(LintAttributeTrailingComment => [LINT_ATTRIBUTE_TRAILING_COMMENT]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[LINT_ATTRIBUTE_TRAILING_COMMENT]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("lint_attribute_trailing_comment", DefaultState::Active)
    {
        return;
    }
    lint_store.register_early_pass(|| Box::new(LintAttributeTrailingComment::new()));
}

const LINT_LEVEL_NAMES: [Symbol; 5] = [sym::allow, sym::expect, sym::warn, sym::deny, sym::forbid];

fn is_lint_level_attribute_name(name: Option<Symbol>) -> bool {
    name.is_some_and(|name| LINT_LEVEL_NAMES.contains(&name))
}

impl EarlyLintPass for LintAttributeTrailingComment {
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        // For an applied `#[cfg_attr(<cond>, <inner>)]`, rustc emits a
        // `cfg_attr_trace` attribute (whose span still covers the
        // original `#[cfg_attr(...)]`) followed by the synthesised
        // `<inner>` attribute(s). The trace's args are an opaque
        // `AttrItemKind::Parsed(CfgAttrTrace)` the public `Attribute`
        // API can't walk, but its outer *span* is available — so we
        // stash it for the synth inner attribute to anchor its
        // comment search to, instead of its own narrower span.
        if attribute.has_name(sym::cfg_attr_trace) {
            // Nested cfg_attr: a `#[cfg_attr(<a>, cfg_attr(<b>, allow(...)))]`
            // produces an outer trace, then an inner trace, then the synth
            // `allow(...)`. The inner trace's span covers only the inner
            // `cfg_attr(<b>, allow(...))` and would miss a trailing comment
            // on the outermost `]`. Preserve the outermost trace when its
            // span already encloses the new one.
            match self.pending_cfg_attr_outer {
                Some(existing) if existing.contains(attribute.span) => {}
                _ => self.pending_cfg_attr_outer = Some(attribute.span),
            }
            return;
        }
        let outer_span = match self.pending_cfg_attr_outer {
            Some(trace_span) if trace_span.contains(attribute.span) => trace_span,
            _ => {
                self.pending_cfg_attr_outer = None;
                attribute.span
            }
        };
        if is_lint_level_attribute_name(attribute.name())
            && let Some(args) = attribute.meta_item_list()
        {
            let emitted = self.check(lint_context, outer_span, attribute.span, &args);
            // One `cfg_attr` is the anchor for one adjacent comment. If
            // it expands to several lint-level synth attrs
            // (`#[cfg_attr(all(), allow(a), warn(b))]`), only the first
            // to actually emit consumes the trace — otherwise each
            // would delete the same comment bytes and rustfix would
            // reject the overlapping edits. Gating on `emitted` keeps
            // the trace live when `check` declines (synth already has a
            // `reason`, or no comment) so a later synth can still find it.
            if emitted && self.pending_cfg_attr_outer == Some(outer_span) {
                self.pending_cfg_attr_outer = None;
            }
        }
    }
}
