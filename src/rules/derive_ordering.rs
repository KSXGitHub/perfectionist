use std::cmp::Ordering;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{Attribute, MetaItemInner};
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, sym};

declare_tool_lint! {
    /// ### What it does
    /// Enforces a project-wide ordering of trait names inside a single
    /// `#[derive(...)]` list. Three styles are configurable via
    /// `style`:
    /// - `preserve` (default) — no-op.
    /// - `alphabetical` — every trait name must be in
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Style {
    /// No-op. The lint emits nothing.
    #[default]
    Preserve,
    /// Every trait name must appear in ASCII-case-insensitive
    /// alphabetical order.
    Alphabetical,
    /// Traits listed in the configured `prefix` come first, in the
    /// listed order; remaining traits are sorted alphabetically
    /// after.
    PrefixThenAlphabetical,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Ordering policy. Defaults to `preserve`, which is a no-op;
    /// a project opts in by setting `alphabetical` or
    /// `prefix_then_alphabetical`.
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
    // Pre-expansion: derives are consumed during macro expansion, so
    // a regular (post-expansion) `EarlyLintPass` no longer sees the
    // `#[derive(...)]` attribute by the time `check_attribute` would
    // be invoked. Running before expansion keeps the attribute
    // tokens intact.
    lint_store.register_pre_expansion_pass(|| Box::new(DeriveOrdering::new()));
}

impl EarlyLintPass for DeriveOrdering {
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if matches!(self.style, Style::Preserve) {
            return;
        }
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
        let desired = self.desired_order(&parsed);
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
            Style::Preserve => unreachable!(),
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

    /// Return a permutation of `0..entries.len()` describing the
    /// desired source order under the configured `style`. A return
    /// value equal to the identity permutation means the entries
    /// are already in order.
    fn desired_order(&self, entries: &[DeriveEntry]) -> Vec<usize> {
        match self.style {
            Style::Preserve => (0..entries.len()).collect(),
            Style::Alphabetical => {
                let mut indices: Vec<usize> = (0..entries.len()).collect();
                // `slice::sort_by` is stable, so equal-key entries
                // retain their original relative order.
                indices.sort_by(|left, right| {
                    ascii_ci_cmp(&entries[*left].final_name, &entries[*right].final_name)
                });
                indices
            }
            Style::PrefixThenAlphabetical => {
                let mut used = vec![false; entries.len()];
                let mut order: Vec<usize> = Vec::with_capacity(entries.len());
                // Walk the configured prefix in order; for each
                // prefix name, find the first not-yet-used entry
                // that matches and append it. An entry that appears
                // in the prefix list but not in the user's derive
                // is silently skipped — the prefix is "preferred
                // ordering", not "required to be present".
                for prefix_name in &self.prefix {
                    for (index, entry) in entries.iter().enumerate() {
                        if !used[index] && entry.final_name == *prefix_name {
                            order.push(index);
                            used[index] = true;
                            break;
                        }
                    }
                }
                let mut rest: Vec<usize> =
                    (0..entries.len()).filter(|index| !used[*index]).collect();
                rest.sort_by(|left, right| {
                    ascii_ci_cmp(&entries[*left].final_name, &entries[*right].final_name)
                });
                order.extend(rest);
                order
            }
        }
    }
}

struct DeriveEntry {
    /// Final segment of the entry's path. `serde::Deserialize` is
    /// represented as `"Deserialize"`. Used for ordering decisions;
    /// the full path is recovered from `span` via the source map
    /// when emitting the suggestion.
    final_name: String,
    /// Source span of the entry, covering its full path text.
    span: Span,
}

/// `Ordering::Less` if `left` precedes `right` in
/// ASCII-case-insensitive alphabetical order. Lowercases byte-by-byte
/// during the comparison rather than allocating a normalised form.
fn ascii_ci_cmp(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}

fn is_identity(permutation: &[usize]) -> bool {
    permutation
        .iter()
        .enumerate()
        .all(|(position, &index)| position == index)
}
