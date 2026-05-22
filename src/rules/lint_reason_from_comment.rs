//! `perfectionist::lint_reason_from_comment` — lift an adjacent
//! `// ...` line comment on a lint-level attribute into the
//! attribute's `reason = "..."` field.
//!
//! Module layout:
//!
//! - [`scan`] — source-text walkers (`find_trailing_comment`,
//!   `find_leading_comment`) and the shared `normalise_comment_text`
//!   that turns a raw `// ...` slice into the rationale-string body.
//! - [`insertion`] — `build_reason_insertion` (the three-layout
//!   args-list edit) and `escape_for_rust_string`.
//! - [`emit`] — `LintReasonFromComment::check` and `::emit`, plus
//!   the `file_span` helper that anchors comment-derived spans to
//!   the same `SyntaxContext` as the attribute they belong to.
//!
//! This flat entry keeps the lint declaration, config / state, the
//! `register_*` functions, and the `EarlyLintPass::check_attribute`
//! driver — including the `cfg_attr_trace` state needed to recover
//! the outer source span for `cfg_attr`-wrapped synth lint-level
//! attrs.

use std::collections::BTreeSet;

use rustc_ast::Attribute;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol, sym};

use crate::common::{DefaultState, resolved_state};

mod emit;
mod insertion;
mod scan;

declare_tool_lint! {
    /// ### What it does
    /// When a lint-level attribute (`#[allow]`, `#[expect]`, `#[warn]`,
    /// `#[deny]`, `#[forbid]`) carries an adjacent line comment that
    /// documents *why* the level was chosen, lifts the comment into
    /// the attribute's `reason = "..."` field and removes the
    /// original comment.
    ///
    /// Two placements count:
    ///
    /// - **Trailing.** A `// ...` comment on the same source line as
    ///   the attribute's closing `]`. Highest confidence.
    /// - **Leading.** A `// ...` comment on the previous source line
    ///   (no blank line between, no other attribute between). Lower
    ///   confidence — the comment may also be documentation for the
    ///   next item.
    ///
    /// Doc comments (`///`, `//!`) and block comments (`/* ... */`)
    /// are out of scope.
    ///
    /// ### Why restrict this?
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
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments)] // matches upstream signature
    /// fn build_fetcher(/* ... */) {}
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    pub perfectionist::LINT_REASON_FROM_COMMENT,
    Warn,
    r#"adjacent comment on a lint-level attribute should be lifted into a `reason = "..."` field"#,
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::lint_reason_from_comment";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Comment placements considered candidates. Subset of
    /// `["trailing", "leading"]`. The trailing placement is the
    /// canonical one and is the highest-confidence case; the leading
    /// placement is lower confidence because the comment may also be
    /// documentation for the next item.
    sites: Vec<Site>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Site {
    /// `// ...` comment on the same source line as the attribute's
    /// closing `]`.
    Trailing,
    /// `// ...` comment on the previous source line, with no blank
    /// line between the comment and the attribute.
    Leading,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sites: vec![Site::Trailing, Site::Leading],
        }
    }
}

pub struct LintReasonFromComment {
    pub(super) sites: BTreeSet<Site>,
    /// Source span of the most recently-visited
    /// `sym::cfg_attr_trace` attribute. rustc replaces a
    /// successfully-applied `#[cfg_attr(<cond>, <inner>)]` with a
    /// trace attribute (whose span covers the original
    /// `#[cfg_attr(...)]`) followed by the synthesised `<inner>`
    /// attributes; the synthesised inner attributes carry source
    /// spans pointing at the inner positions *within* the
    /// cfg_attr source, which is too narrow to scan for adjacent
    /// comments. Stashing the trace's outer span here lets the
    /// next synth lint-level attribute use it for the
    /// comment-search anchor. Cleared when we visit an attribute
    /// whose span lies outside the trace's range — the trace's
    /// influence ends as soon as the AST walks past it.
    pending_cfg_attr_outer: Option<Span>,
}

impl LintReasonFromComment {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            sites: config.sites.into_iter().collect(),
            pending_cfg_attr_outer: None,
        }
    }
}

impl_lint_pass!(LintReasonFromComment => [LINT_REASON_FROM_COMMENT]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[LINT_REASON_FROM_COMMENT]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("lint_reason_from_comment", DefaultState::Active)
    {
        return;
    }
    lint_store.register_early_pass(|| Box::new(LintReasonFromComment::new()));
}

const LINT_LEVEL_NAMES: [Symbol; 5] = [sym::allow, sym::expect, sym::warn, sym::deny, sym::forbid];

fn is_lint_level_attribute_name(name: Option<Symbol>) -> bool {
    name.is_some_and(|name| LINT_LEVEL_NAMES.contains(&name))
}

impl EarlyLintPass for LintReasonFromComment {
    /// `EarlyLintPass::check_attribute` visits each syntactic
    /// attribute once. A bare `#[allow(...)]` arrives directly; a
    /// `cfg_attr`-wrapped `#[cfg_attr(<cond>, allow(...))]` is split
    /// by rustc into a `sym::cfg_attr_trace` attribute (carrying the
    /// outer span) and one or more synth `#[allow(...)]` attributes
    /// — see the cfg_attr-trace handling below.
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if self.sites.is_empty() {
            return;
        }
        // rustc replaces a `#[cfg_attr(<cond>, <inner>)]` whose
        // condition evaluates true with a *trace* attribute named
        // `sym::cfg_attr_trace` (whose span still covers the original
        // `#[cfg_attr(...)]`) followed by the synthesised `<inner>`
        // attribute(s). The trace's `args` are already parsed into a
        // private `AttrItemKind::Parsed(CfgAttrTrace)` payload that
        // is opaque to the public `Attribute` API — there's no way
        // to walk its inner meta items directly. The trace's outer
        // *span* is still available, though, so we stash it as we
        // visit the trace and use it as the comment-search anchor
        // for the synth lint-level attributes that follow.
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
            // A cfg_attr trace is one comment anchor for *one* adjacent
            // source comment; if the cfg_attr expands into multiple
            // lint-level synth attributes (e.g.
            // `#[cfg_attr(all(), allow(a), warn(b))]`), only the first
            // one to actually lift the comment should consume the
            // trace — otherwise every synth attr would emit a duplicate
            // suggestion whose `delete_span` overlaps the others on the
            // same comment bytes, and rustfix would refuse to apply
            // the conflicting edits. Conversely, if `check` declined
            // to emit (e.g. the first synth already carries a `reason`
            // field, or no adjacent comment was found), the trace must
            // stay live so the next synth in the same cfg_attr can
            // still find it.
            if emitted && self.pending_cfg_attr_outer == Some(outer_span) {
                self.pending_cfg_attr_outer = None;
            }
        }
    }
}
