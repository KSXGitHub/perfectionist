use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{Attribute, MetaItem, MetaItemInner, MetaItemKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Symbol, sym};

declare_tool_lint! {
    /// ### What it does
    /// Flags lint-control attributes (`allow`, `warn`, `deny`,
    /// `forbid`, `expect`, including under `cfg_attr`) whose lint
    /// name starts with `perfectionist::` but does not name a lint
    /// this plugin actually registers.
    ///
    /// ### Why is this bad?
    /// Typos and stale references in `#[allow(perfectionist::...)]`
    /// silently neutralise the suppression they were written for.
    /// rustc's own `unknown_lints` covers tool-prefixed names
    /// inconsistently; this rule fills the gap and offers a
    /// "did you mean" hint against the registered set.
    ///
    /// ### Example
    /// ```rust,ignore
    /// #[allow(perfectionist::unicode_ellipsis_in_comment)] // typo
    /// fn legacy() {}
    /// ```
    /// Use instead:
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
#[serde(default, rename_all = "snake_case")]
struct Config {
    suggestion_distance: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            suggestion_distance: 2,
        }
    }
}

pub struct UnknownPerfectionistLints {
    suggestion_distance: usize,
    known: Vec<String>,
}

impl UnknownPerfectionistLints {
    fn new(known: Vec<String>) -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            suggestion_distance: config.suggestion_distance,
            known,
        }
    }
}

impl_lint_pass!(UnknownPerfectionistLints => [UNKNOWN_PERFECTIONIST_LINTS]);

/// Register only the lint declaration. Call this from `register_lints`
/// alongside the other rule modules' registration calls; the early pass
/// itself is installed separately by [`register_pass`] once every lint has
/// been registered, so the pass can read the full set out of the store.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNKNOWN_PERFECTIONIST_LINTS]);
}

/// Install the early pass. Must be called *after* every other rule module
/// has registered its lints, since the pass snapshots the registered
/// `perfectionist::*` names from `lint_store` at construction time.
pub fn register_pass(lint_store: &mut LintStore) {
    let registered: Vec<String> = collect_registered_names(lint_store);
    lint_store
        .register_early_pass(move || Box::new(UnknownPerfectionistLints::new(registered.clone())));
}

fn collect_registered_names(lint_store: &LintStore) -> Vec<String> {
    let prefix = format!("{TOOL_NAME}::");
    lint_store
        .get_lints()
        .iter()
        .filter_map(|lint| {
            // `Lint::name` is the upper-case macro identifier
            // (`perfectionist::UNICODE_ELLIPSIS_IN_COMMENTS`); `name_lower()`
            // returns the snake-case form rustc surfaces in diagnostics and
            // attribute references (`perfectionist::unicode_ellipsis_in_comments`).
            let lower = lint.name_lower();
            lower.strip_prefix(&prefix).map(str::to_owned)
        })
        .collect()
}

impl EarlyLintPass for UnknownPerfectionistLints {
    fn check_attribute(&mut self, cx: &EarlyContext<'_>, attr: &Attribute) {
        if is_lint_control_attr(attr) {
            if let Some(items) = attr.meta_item_list() {
                self.check_items(cx, &items);
            }
        } else if attr.has_name(sym::cfg_attr) {
            let Some(items) = attr.meta_item_list() else {
                return;
            };
            for inner in items.iter().skip(1) {
                let Some(meta) = inner.meta_item() else {
                    continue;
                };
                if !is_lint_control_meta(meta) {
                    continue;
                }
                let MetaItemKind::List(nested) = &meta.kind else {
                    continue;
                };
                self.check_items(cx, nested);
            }
        }
    }
}

const LINT_CONTROL_NAMES: [Symbol; 5] =
    [sym::allow, sym::warn, sym::deny, sym::forbid, sym::expect];

fn is_lint_control_attr(attr: &Attribute) -> bool {
    LINT_CONTROL_NAMES.iter().any(|name| attr.has_name(*name))
}

fn is_lint_control_meta(meta: &MetaItem) -> bool {
    LINT_CONTROL_NAMES.iter().any(|name| meta.has_name(*name))
}

impl UnknownPerfectionistLints {
    fn check_items(&self, cx: &EarlyContext<'_>, items: &[MetaItemInner]) {
        for inner in items {
            let Some(meta) = inner.meta_item() else {
                continue;
            };
            self.check_lint_name(cx, meta);
        }
    }

    fn check_lint_name(&self, cx: &EarlyContext<'_>, meta: &MetaItem) {
        let segments = &meta.path.segments;
        let Some(first) = segments.first() else {
            return;
        };
        if first.ident.name.as_str() != TOOL_NAME {
            return;
        }
        let trailing: Vec<&str> = segments[1..]
            .iter()
            .map(|s| s.ident.name.as_str())
            .collect();
        match trailing.as_slice() {
            [name] if self.is_known(name) => {}
            [name] => self.report(cx, meta, name),
            [] => self.report_no_name(cx, meta),
            _ => {
                let candidate = trailing.join("_");
                self.report(cx, meta, &candidate);
            }
        }
    }

    fn is_known(&self, name: &str) -> bool {
        self.known.iter().any(|k| k == name)
    }

    fn nearest(&self, candidate: &str) -> Option<&str> {
        if self.suggestion_distance == 0 {
            return None;
        }
        let mut best: Option<(&str, usize)> = None;
        for k in &self.known {
            let d = levenshtein(candidate, k);
            if d <= self.suggestion_distance && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((k.as_str(), d));
            }
        }
        best.map(|(name, _)| name)
    }

    fn report(&self, cx: &EarlyContext<'_>, meta: &MetaItem, candidate: &str) {
        let printed = path_to_string(meta);
        let suggestion = self.nearest(candidate);
        span_lint_and_then(
            cx,
            UNKNOWN_PERFECTIONIST_LINTS,
            meta.span,
            format!("unknown lint: `{printed}`"),
            |diag| {
                if let Some(s) = suggestion {
                    diag.help(format!("did you mean `{TOOL_NAME}::{s}`?"));
                }
            },
        );
    }

    fn report_no_name(&self, cx: &EarlyContext<'_>, meta: &MetaItem) {
        span_lint_and_then(
            cx,
            UNKNOWN_PERFECTIONIST_LINTS,
            meta.span,
            format!("unknown lint: `{TOOL_NAME}` is a tool prefix, not a lint name"),
            |_| {},
        );
    }
}

fn path_to_string(meta: &MetaItem) -> String {
    let mut out = String::new();
    for (i, seg) in meta.path.segments.iter().enumerate() {
        if i > 0 {
            out.push_str("::");
        }
        out.push_str(seg.ident.name.as_str());
    }
    out
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}
