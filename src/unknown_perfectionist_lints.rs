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
    /// #[allow(perfectionist::qualified_path)] // typo
    /// fn legacy() {}
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// #[allow(perfectionist::qualified_paths)]
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
    extra_known_names: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            suggestion_distance: 2,
            extra_known_names: Vec::new(),
        }
    }
}

pub struct UnknownPerfectionistLints {
    suggestion_distance: usize,
    known: Vec<String>,
}

impl UnknownPerfectionistLints {
    fn new(registered_names: &'static [&'static str]) -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut known: Vec<String> = registered_names.iter().map(|s| (*s).to_owned()).collect();
        for extra in config.extra_known_names {
            if !known.iter().any(|n| n == &extra) {
                known.push(extra);
            }
        }
        Self {
            suggestion_distance: config.suggestion_distance,
            known,
        }
    }
}

impl_lint_pass!(UnknownPerfectionistLints => [UNKNOWN_PERFECTIONIST_LINTS]);

pub fn register(lint_store: &mut LintStore, registered_names: &'static [&'static str]) {
    lint_store.register_lints(&[UNKNOWN_PERFECTIONIST_LINTS]);
    lint_store
        .register_early_pass(move || Box::new(UnknownPerfectionistLints::new(registered_names)));
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
                    diag.help(format!("did you mean `perfectionist::{s}`?"));
                }
            },
        );
    }

    fn report_no_name(&self, cx: &EarlyContext<'_>, meta: &MetaItem) {
        span_lint_and_then(
            cx,
            UNKNOWN_PERFECTIONIST_LINTS,
            meta.span,
            "unknown lint: `perfectionist` is a tool prefix, not a lint name",
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
