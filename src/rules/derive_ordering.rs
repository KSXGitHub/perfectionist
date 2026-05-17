//! `perfectionist::derive_ordering` — enforce a project-wide ordering
//! of trait names inside a single `#[derive(...)]` list.
//!
//! The ordering algorithm itself lives in [`ordering`]; this file
//! owns the lint declaration, the configuration, and the early
//! pre-expansion pass that runs it.

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{Attribute, MetaItemInner};
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::sym;

mod ordering;

use ordering::{DeriveEntry, Style, desired_order, is_identity};

use crate::common::{DefaultState, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// Enforces a project-wide ordering of trait names inside a single
    /// `#[derive(...)]` list. Two styles are configurable via
    /// `style`:
    /// - `alphabetical` (default) — every trait name must be in
    ///   ASCII-case-insensitive alphabetical order.
    /// - `prefix_then_alphabetical` — the configured `prefix` list of
    ///   traits goes first, in the listed order; remaining traits are
    ///   sorted alphabetically after.
    ///
    /// Trait matching is by the final path segment, so
    /// `serde::Deserialize` is matched as `Deserialize`. The lint
    /// does not police how derives are partitioned across multiple
    /// `#[derive(...)]` lines — that's a layout decision left to the
    /// author.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. The
    /// trait order inside `#[derive(...)]` has no semantic effect:
    /// `#[derive(Debug, Clone)]` and `#[derive(Clone, Debug)]`
    /// produce identical impls. A project-wide convention makes
    /// derive lists scan uniformly across the codebase. `cargo fmt`
    /// does not reorder derives, so this lint is the only mechanism
    /// for enforcing one.
    ///
    /// The opinion is opt-in: a project that doesn't want to commit
    /// to a single ordering shouldn't have to set anything. The rule
    /// is therefore inactive by default — enable it per crate by
    /// adding to `dylint.toml`:
    ///
    /// ```toml
    /// [perfectionist]
    /// enable = ["derive_ordering"]
    /// ```
    ///
    /// ### Example
    /// Under `style = "alphabetical"`:
    /// ```rust,ignore
    /// #[derive(Debug, Clone, Copy)]
    /// struct Point;
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// #[derive(Clone, Copy, Debug)]
    /// struct Point;
    /// ```
    pub perfectionist::DERIVE_ORDERING,
    Warn,
    "trait names in a `#[derive(...)]` list are not in the configured order",
    report_in_external_macro: false
}

/// Off by default — enable it in `dylint.toml` via the crate-wide
/// `[perfectionist] enable = ["derive_ordering"]` (or the
/// `[[perfectionist.enable]]` array-of-tables form). Read by
/// `register_pass` below; gen-docs picks the constant up via syn
/// to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Disabled;

const CONFIG_KEY: &str = "perfectionist::derive_ordering";

/// Default `prefix` list for the `prefix_then_alphabetical` style.
/// Common standard-library derives first (in the order one would
/// typically read them), then the comparison-trait quartet, then
/// `Hash`. The default applies only when `style =
/// "prefix_then_alphabetical"`; under any other style the prefix is
/// unused.
const DEFAULT_PREFIX: &[&str] = &[
    "Debug",
    "Default",
    "Clone",
    "Copy",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Hash",
];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Ordering policy. Defaults to `alphabetical`; set
    /// `prefix_then_alphabetical` to pin a configured `prefix` list
    /// of traits ahead of the alphabetised tail.
    style: Style,
    /// Trait names that must appear first under the
    /// `prefix_then_alphabetical` style, in the order they should
    /// appear. Ignored under other styles. Matched by the final
    /// path segment, so a configured `"Debug"` matches both
    /// `Debug` and `std::fmt::Debug` written in the source.
    prefix: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Style::default(),
            prefix: DEFAULT_PREFIX
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        }
    }
}

pub struct DeriveOrdering {
    style: Style,
    prefix: Vec<String>,
}

impl DeriveOrdering {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            style: config.style,
            prefix: config.prefix,
        }
    }
}

impl_lint_pass!(DeriveOrdering => [DERIVE_ORDERING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[DERIVE_ORDERING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Disabled = resolved_state("derive_ordering", DEFAULT_STATE) {
        return;
    }
    // Pre-expansion: derives are consumed during macro expansion, so
    // a regular (post-expansion) `EarlyLintPass` no longer sees the
    // `#[derive(...)]` attribute by the time `check_attribute` would
    // be invoked. Running before expansion keeps the attribute
    // tokens intact.
    lint_store.register_pre_expansion_pass(|| Box::new(DeriveOrdering::new()));
}

impl EarlyLintPass for DeriveOrdering {
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if !attribute.has_name(sym::derive) {
            return;
        }
        let Some(entries) = attribute.meta_item_list() else {
            return;
        };
        self.check_derive_list(lint_context, &entries);
    }
}

impl DeriveOrdering {
    fn check_derive_list(&self, lint_context: &EarlyContext<'_>, entries: &[MetaItemInner]) {
        // Fewer than two entries can never be in the wrong order: a
        // zero- or one-entry list is vacuously sorted. The same
        // bail-out also short-circuits the very common case of
        // `#[derive(Foo)]` with a single trait. Two-entry lists
        // *can* be out of order and are analysed in full below.
        if entries.len() < 2 {
            return;
        }
        let mut parsed: Vec<DeriveEntry> = Vec::with_capacity(entries.len());
        for entry in entries {
            // `MetaItemInner::Lit` cannot legally appear inside a
            // `#[derive(...)]` list — derive takes paths only. If we
            // see one, the attribute is malformed; bail rather than
            // emit a confusing suggestion.
            let Some(meta_item) = entry.meta_item() else {
                return;
            };
            let Some(last_segment) = meta_item.path.segments.last() else {
                return;
            };
            parsed.push(DeriveEntry {
                final_name: last_segment.ident.name.as_str().to_owned(),
                span: entry.span(),
            });
        }
        let desired = desired_order(self.style, &self.prefix, &parsed);
        if is_identity(&desired) {
            return;
        }
        let source_map = lint_context.sess().source_map();
        let mut snippets: Vec<String> = Vec::with_capacity(desired.len());
        for &index in &desired {
            match source_map.span_to_snippet(parsed[index].span) {
                Ok(snippet) => snippets.push(snippet),
                // The source map should always be able to produce a
                // snippet for an attribute span (derives only appear
                // in real source files, not synthesised macro
                // expansions for which this would fail), but if the
                // snippet is unavailable for any reason, fall back
                // to the final-segment name. The suggestion stays
                // applicable; only the path prefix is lost.
                Err(_) => snippets.push(parsed[index].final_name.clone()),
            }
        }
        let new_text = snippets.join(", ");
        let replace_span = parsed[0].span.with_hi(parsed[parsed.len() - 1].span.hi());
        // The suggestion replaces the entire first-to-last span with a
        // single-line `entry, entry` reconstruction, so any inline
        // whitespace, line breaks, or comments between entries are
        // lost on apply. For a single-line derive with no inline
        // comments that's exactly the shape rustfmt produces, so
        // `MachineApplicable` is safe and matches the planning spec.
        // If the derive spans multiple lines or contains inline
        // comment tokens between entries — where the apply would
        // visibly flatten the list or drop user comments — downgrade
        // to `MaybeIncorrect` so `cargo fix` does not silently squash
        // the formatting; the suggestion still surfaces in IDE
        // diagnostics for manual review. Derive entries are paths
        // (`Foo::Bar`) and cannot themselves contain `//` or `/*`,
        // so a substring check on the replaced snippet is sufficient.
        let replace_snippet = source_map.span_to_snippet(replace_span).ok();
        let has_inline_comment = replace_snippet
            .as_deref()
            .is_some_and(|snippet| snippet.contains("//") || snippet.contains("/*"));
        let applicability = if source_map.is_multiline(replace_span) || has_inline_comment {
            Applicability::MaybeIncorrect
        } else {
            Applicability::MachineApplicable
        };
        let message = match self.style {
            Style::Alphabetical => {
                "derive list is not in ASCII-case-insensitive alphabetical order"
            }
            Style::PrefixThenAlphabetical => {
                "derive list does not match the configured `prefix_then_alphabetical` order"
            }
        };
        span_lint_and_then(
            lint_context,
            DERIVE_ORDERING,
            replace_span,
            message,
            |diagnostic| {
                diagnostic.span_suggestion(
                    replace_span,
                    "reorder the derive list",
                    new_text,
                    applicability,
                );
            },
        );
    }
}
