use crate::common::{DefaultState, render_meta_path, resolved_state};
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{Attribute, MetaItem, MetaItemInner, MetaItemKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Symbol, sym};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags lint-control attributes (`allow`, `warn`, `deny`,
    /// `forbid`, `expect`, including under `cfg_attr`) whose lint
    /// name starts with `perfectionist::` but does not name a lint
    /// this plugin actually registers.
    ///
    /// ### Why is this bad?
    ///
    /// Typos and stale references in `#[allow(perfectionist::...)]`
    /// silently neutralise the suppression they were written for.
    /// rustc's own `unknown_lints` covers tool-prefixed names
    /// inconsistently; this rule fills the gap and offers a
    /// "did you mean" hint against the registered set.
    ///
    /// ### Example
    ///
    /// **Bad:**
    ///
    /// ```rust,ignore
    /// #[allow(perfectionist::unicode_ellipsis_in_comment)] // typo
    /// fn legacy() {}
    /// ```
    ///
    /// **Good:**
    ///
    /// ```rust,ignore
    /// #[allow(perfectionist::unicode_ellipsis_in_comments)]
    /// fn legacy() {}
    /// ```
    pub perfectionist::UNKNOWN_PERFECTIONIST_LINTS,
    Warn,
    "lint-control attribute references a `perfectionist::*` lint that this plugin does not register",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::unknown_perfectionist_lints";
const TOOL_NAME: &str = "perfectionist";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Maximum Levenshtein edit distance between an unknown
    /// `perfectionist::*` name and a registered lint for the lint to
    /// emit a "did you mean" suggestion. Defaults to `2`, which
    /// catches single-character typos and short transpositions
    /// without producing wild guesses. Set to `0` to disable
    /// suggestions entirely.
    suggestion_distance: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            suggestion_distance: 2,
        }
    }
}

pub struct UnknownPerfectionistLints {
    suggestion_distance: u8,
    registered_lints: Vec<String>,
}

impl UnknownPerfectionistLints {
    fn new(registered_lints: Vec<String>) -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            suggestion_distance: config.suggestion_distance,
            registered_lints,
        }
    }
}

impl_lint_pass!(UnknownPerfectionistLints => [UNKNOWN_PERFECTIONIST_LINTS]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNKNOWN_PERFECTIONIST_LINTS]);
}

/// Install this rule's early pass. Must be called *after* every rule module
/// has registered its lints, since the pass snapshots the registered
/// `perfectionist::*` names from `lint_store` at construction time.
pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("unknown_perfectionist_lints", DefaultState::Active)
    {
        return;
    }
    let registered_lints: Vec<String> = collect_registered_lint_names(lint_store);
    lint_store.register_early_pass(move || {
        Box::new(UnknownPerfectionistLints::new(registered_lints.clone()))
    });
}

fn collect_registered_lint_names(lint_store: &LintStore) -> Vec<String> {
    let tool_prefix = format!("{TOOL_NAME}::");
    lint_store
        .get_lints()
        .iter()
        .filter_map(|lint| {
            // `Lint::name` is the upper-case macro identifier
            // (`perfectionist::UNICODE_ELLIPSIS_IN_COMMENTS`); `name_lower()`
            // returns the snake-case form rustc surfaces in diagnostics and
            // attribute references (`perfectionist::unicode_ellipsis_in_comments`).
            let lower_name = lint.name_lower();
            lower_name.strip_prefix(&tool_prefix).map(str::to_owned)
        })
        .collect()
}

impl EarlyLintPass for UnknownPerfectionistLints {
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if is_lint_level_attribute(attribute) {
            if let Some(lint_names) = attribute.meta_item_list() {
                self.check_lint_name_list(lint_context, &lint_names);
            }
        } else if attribute.has_name(sym::cfg_attr) {
            let Some(cfg_attr_args) = attribute.meta_item_list() else {
                return;
            };
            for wrapped_attribute in cfg_attr_args.iter().skip(1) {
                let Some(wrapped_meta_item) = wrapped_attribute.meta_item() else {
                    continue;
                };
                if !is_lint_level_meta_item(wrapped_meta_item) {
                    continue;
                }
                let MetaItemKind::List(lint_names) = &wrapped_meta_item.kind else {
                    continue;
                };
                self.check_lint_name_list(lint_context, lint_names);
            }
        }
    }
}

const LINT_LEVEL_ATTRIBUTE_NAMES: [Symbol; 5] =
    [sym::allow, sym::warn, sym::deny, sym::forbid, sym::expect];

fn is_lint_level_attribute(attribute: &Attribute) -> bool {
    LINT_LEVEL_ATTRIBUTE_NAMES
        .iter()
        .any(|name| attribute.has_name(*name))
}

fn is_lint_level_meta_item(meta_item: &MetaItem) -> bool {
    LINT_LEVEL_ATTRIBUTE_NAMES
        .iter()
        .any(|name| meta_item.has_name(*name))
}

impl UnknownPerfectionistLints {
    fn check_lint_name_list(&self, lint_context: &EarlyContext<'_>, lint_names: &[MetaItemInner]) {
        for lint_name in lint_names {
            let Some(meta_item) = lint_name.meta_item() else {
                continue;
            };
            self.check_lint_name(lint_context, meta_item);
        }
    }

    fn check_lint_name(&self, lint_context: &EarlyContext<'_>, meta_item: &MetaItem) {
        let segments = &meta_item.path.segments;
        let Some(first_segment) = segments.first() else {
            return;
        };
        if first_segment.ident.name.as_str() != TOOL_NAME {
            return;
        }
        let segments_after_tool: Vec<&str> = segments[1..]
            .iter()
            .map(|segment| segment.ident.name.as_str())
            .collect();
        match segments_after_tool.as_slice() {
            [name] if self.is_registered(name) => {}
            [name] => self.report(lint_context, meta_item, name),
            [] => self.report_no_name(lint_context, meta_item),
            _ => {
                let candidate = segments_after_tool.join("_");
                self.report(lint_context, meta_item, &candidate);
            }
        }
    }

    fn is_registered(&self, name: &str) -> bool {
        self.registered_lints
            .iter()
            .any(|registered| registered == name)
    }

    /// The help line to attach to an unknown name, if there is anything
    /// useful to say about it.
    fn help_for(&self, candidate: &str) -> Option<String> {
        // No registered lint name holds a non-ASCII character, so a
        // candidate that does is not a near-miss of one and there is
        // nothing to guess at. The character itself is the answer: a
        // homoglyph is invisible in the source, and the codepoint is
        // what identifies it. rustc words its own reports of such a
        // character the same way, down to `'о' (U+043E)`.
        if let Some(non_ascii) = candidate.chars().find(|character| !character.is_ascii()) {
            let codepoint = u32::from(non_ascii);
            return Some(format!(
                "contains a non-ASCII character: '{non_ascii}' (U+{codepoint:04X})"
            ));
        }
        let suggested_name = self.find_closest_match(candidate)?;
        Some(format!("did you mean `{TOOL_NAME}::{suggested_name}`?"))
    }

    /// The registered lint closest to `candidate`, within
    /// `suggestion_distance` edits of it. `candidate` must be ASCII;
    /// [`Self::help_for`] rules out everything else beforehand.
    fn find_closest_match(&self, candidate: &str) -> Option<&str> {
        if self.suggestion_distance == 0 {
            return None;
        }
        let mut closest: Option<(&str, usize)> = None;
        for registered in &self.registered_lints {
            let distance = levenshtein(candidate.as_bytes(), registered.as_bytes());
            if distance <= self.suggestion_distance as usize
                && closest.is_none_or(|(_, closest_distance)| distance < closest_distance)
            {
                closest = Some((registered.as_str(), distance));
            }
        }
        closest.map(|(name, _)| name)
    }

    fn report(&self, lint_context: &EarlyContext<'_>, meta_item: &MetaItem, candidate: &str) {
        let path_text = render_meta_path(meta_item);
        let help = self.help_for(candidate);
        span_lint_and_then(
            lint_context,
            UNKNOWN_PERFECTIONIST_LINTS,
            meta_item.span,
            format!("unknown lint: `{path_text}`"),
            |diagnostic| {
                if let Some(help) = help {
                    diagnostic.help(help);
                }
            },
        );
    }

    fn report_no_name(&self, lint_context: &EarlyContext<'_>, meta_item: &MetaItem) {
        span_lint_and_then(
            lint_context,
            UNKNOWN_PERFECTIONIST_LINTS,
            meta_item.span,
            format!("unknown lint: `{TOOL_NAME}` is a tool prefix, not a lint name"),
            |_| {},
        );
    }
}

/// The Levenshtein edit distance between `left` and `right`: the fewest
/// single-byte insertions, deletions, and substitutions that turn one
/// into the other.
///
/// Both sides are lint names, and every lint name this plugin registers
/// is ASCII — a candidate that is not never reaches here — so bytes are
/// characters, and `str::as_bytes` needs neither decoding nor a
/// `Vec<char>` to compare them.
fn levenshtein(left: &[u8], right: &[u8]) -> usize {
    let left_len = left.len();
    let right_len = right.len();
    if left_len == 0 {
        return right_len;
    }
    if right_len == 0 {
        return left_len;
    }
    let mut previous_row: Vec<usize> = (0..=right_len).collect();
    let mut current_row: Vec<usize> = vec![0; right_len + 1];
    for i in 1..=left_len {
        current_row[0] = i;
        for j in 1..=right_len {
            let substitution_cost = usize::from(left[i - 1] != right[j - 1]);
            current_row[j] = (previous_row[j] + 1)
                .min(current_row[j - 1] + 1)
                .min(previous_row[j - 1] + substitution_cost);
        }
        core::mem::swap(&mut previous_row, &mut current_row);
    }
    previous_row[right_len]
}

#[cfg(test)]
mod tests;
