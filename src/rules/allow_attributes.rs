use crate::common::{DefaultState, render_meta_path, resolve_string_set};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::{span_lint_and_help, span_lint_and_then};
use clippy_utils::is_from_proc_macro;
use clippy_utils::source::{indent_of, snippet_opt};
use rustc_ast::{AttrStyle, Attribute, Item, ItemKind, MetaItem, MetaItemInner, MetaItemKind};
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, Lint, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol, sym};
use std::collections::{BTreeSet, HashSet};

#[cfg(test)]
mod tests;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags `#[allow(<lints>)]` when every named lint fires
    /// deterministically — a built-in rustc lint (not on the exempt
    /// list), a `clippy::*` / `rustdoc::*` lint, or a tool-namespaced
    /// lint such as `perfectionist::*`. Such a suppression can be
    /// resolved two ways:
    ///
    /// 1. **Remove** it — when the lint can no longer fire at the site
    ///    (a dead suppression, e.g. `clippy::too_many_arguments` left on
    ///    a function that has since shed its arguments).
    /// 2. **Replace** it with `#[expect]` — when the lint still fires and
    ///    you want the suppression to report itself the moment it stops.
    ///
    /// If the attribute also names a lint the rule leaves alone (an
    /// exempt or unknown lint), only the deterministic names are
    /// resolved; the rest stay under `#[allow]`.
    ///
    /// Crate- and module-level scopes (`#![allow(...)]`, and outer
    /// `#[allow(...)]` on a `mod` item) are left alone by default,
    /// because a `cfg`-conditional body inside the scope may fire the
    /// lint in one configuration and not another — set
    /// `apply_to_outer_scopes = true` to opt in.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// A suppression often outlives the problem it suppressed.
    /// `#[allow]` stays silent forever, including after the underlying
    /// issue is resolved, so a project accumulates stale `#[allow]`
    /// attributes that no longer apply. `#[expect]` emits
    /// `unfulfilled_lint_expectations` the moment the named lint stops
    /// triggering at the site — exactly when the suppression becomes
    /// dead — so routine compilation tells the author to remove it.
    /// Every `#[expect]` is also a self-test that the lint *does* fire
    /// at the site, so a future refactor that inadvertently fixes the
    /// issue is observed rather than hidden.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::allow_attributes` (`restriction`, off by default)
    /// also pushes `#[allow]` towards `#[expect]`, but rewrites
    /// *every* `#[allow]` indiscriminately, and only towards `#[expect]`.
    /// This rule flags only the lints that fire **deterministically**,
    /// leaving lint groups and conditionally-firing lints under
    /// `#[allow]`. Crucially, it does not assume `#[expect]` is always
    /// the answer: a deterministic `#[allow]` may already be dead, in
    /// which case `#[expect]` would be unfulfilled and removal is the
    /// right fix, so the rule offers both. Reach for this rule for that
    /// precision, or `clippy::allow_attributes` for a blunt crate-wide
    /// sweep.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments, reason = "matches pnpm's signature")]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// #[expect(clippy::too_many_arguments, reason = "matches pnpm's signature")]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    pub perfectionist::ALLOW_ATTRIBUTES,
    Warn,
    "`#[allow]` for a deterministically-firing lint should be removed or be `#[expect]`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::allow_attributes";

/// The lints `#[allow]` keeps because they cannot be relied on to fire
/// deterministically: the `cfg`-conditional `unused_*` and
/// reachability lints. Each can fire under one `cfg` arm and stay
/// silent under another, so a mechanical `expect` rewrite would break
/// the build in the silent arm.
const DEFAULT_EXEMPT_LINTS: &[&str] = &[
    "dead_code",
    "unused_imports",
    "unused_macros",
    "unused_variables",
    "unused_mut",
    "unused_assignments",
    "unused_must_use",
    "unreachable_code",
];

/// Clippy lint *group* names. A group fires only if some member lint
/// fires, so `#[expect(clippy::<group>)]` is unfulfilled wherever no
/// member triggers — the same non-determinism [`DEFAULT_EXEMPT_LINTS`]
/// guards against. Unlike rustc's bare groups (`unused`, etc.), these are
/// not in the [`LintStore`] snapshot (clippy is not loaded during a
/// `cargo dylint` run), so they are listed explicitly.
const CLIPPY_LINT_GROUPS: &[&str] = &[
    "all",
    "cargo",
    "complexity",
    "correctness",
    "deprecated",
    "nursery",
    "pedantic",
    "perf",
    "restriction",
    "style",
    "suspicious",
];

/// Rustdoc lint *group* names, treated like [`CLIPPY_LINT_GROUPS`].
const RUSTDOC_LINT_GROUPS: &[&str] = &["all"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Extra lints to exempt, on top of the built-in default set (the
    /// `cfg`-conditional `unused_*` / reachability lints). Names are
    /// matched against the fully-namespaced lint name shown in
    /// diagnostics (e.g. `clippy::too_many_arguments`). Merged with the
    /// defaults rather than replacing them.
    extra_exempt_lints: Vec<String>,
    /// Lints to drop from the exempt set, even if they appear in the
    /// built-in defaults or in `extra_exempt_lints`. Use this to opt a
    /// default exemption back into rewriting (e.g. `["dead_code"]` in a
    /// project with no `cfg`-gated dead code).
    ignore_exempt_lints: Vec<String>,
    /// When true, also rewrite crate-level `#![allow(...)]` and
    /// module-level `#[allow(...)]` attributes. Default `false`
    /// because `cfg`-conditional bodies inside the scope are common.
    apply_to_outer_scopes: bool,
    /// When false, only `clippy::*`, `rustdoc::*`, and built-in lints
    /// are rewritten; other tool namespaces (`perfectionist::*` and
    /// similar) are left alone. Default `true` — a tool namespace's
    /// lints are assumed to fire deterministically like a built-in, so
    /// `perfectionist::*` (and similar) are rewritten by default.
    apply_to_tool_namespaces: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extra_exempt_lints: Vec::new(),
            ignore_exempt_lints: Vec::new(),
            apply_to_outer_scopes: false,
            apply_to_tool_namespaces: true,
        }
    }
}

pub struct AllowAttributes {
    exempt_lints: BTreeSet<String>,
    apply_to_outer_scopes: bool,
    apply_to_tool_namespaces: bool,
    /// Snapshot of every built-in rustc lint name (no tool prefix) that
    /// was registered in the [`LintStore`] when the pass was installed.
    /// A bare lint name in an `#[allow]` is "built-in, deterministic"
    /// only if it is in this set; anything else is an unknown name that
    /// might belong to a procedural plugin that fires conditionally, so
    /// it is left under `#[allow]`. Lint *group* names (`unused`,
    /// `nonstandard_style`, and similar) are deliberately absent — a
    /// group fires if *any* member fires, the same non-deterministic
    /// shape the exempt list guards against, so groups are never
    /// rewritten.
    builtin_lints: HashSet<String>,
    /// Spans of attributes that sit on a `mod` item, collected from
    /// [`EarlyLintPass::check_item`] so [`EarlyLintPass::check_attribute`]
    /// can recognise the module-scope case and skip it unless
    /// `apply_to_outer_scopes` is set. `check_item` for an item runs
    /// before that item's own attributes are visited, so the span is
    /// already recorded by the time `check_attribute` consults it.
    module_attr_spans: HashSet<Span>,
}

impl AllowAttributes {
    fn new(builtin_lints: HashSet<String>) -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            exempt_lints: resolve_string_set(
                DEFAULT_EXEMPT_LINTS,
                config.extra_exempt_lints,
                config.ignore_exempt_lints,
            ),
            apply_to_outer_scopes: config.apply_to_outer_scopes,
            apply_to_tool_namespaces: config.apply_to_tool_namespaces,
            builtin_lints,
            module_attr_spans: HashSet::new(),
        }
    }
}

impl_lint_pass!(AllowAttributes => [ALLOW_ATTRIBUTES]);

impl Register for rule::AllowAttributes {
    /// Active by default. The rewrite is conservative — it only fires
    /// when every named lint is known to fire deterministically — so a
    /// baseline policy is not presumptuous.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[ALLOW_ATTRIBUTES]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let builtin_lints = collect_builtin_lint_names(lint_store);
        lint_store
            .register_early_pass(move || Box::new(AllowAttributes::new(builtin_lints.clone())));
    }
}

/// Collect every registered lint whose printed name carries no tool
/// prefix — the built-in rustc lints. `clippy::*` / `rustdoc::*` /
/// `perfectionist::*` lints are recognised structurally by their path
/// prefix instead, so they are filtered out here.
fn collect_builtin_lint_names(lint_store: &LintStore) -> HashSet<String> {
    lint_store
        .get_lints()
        .iter()
        .map(|lint: &&Lint| lint.name_lower())
        .filter(|name| !name.contains("::"))
        .collect()
}

const ALLOW: Symbol = sym::allow;

impl EarlyLintPass for AllowAttributes {
    /// Record the spans of a module item's own attributes so
    /// [`Self::check_attribute`] can identify the module-scope case.
    fn check_item(&mut self, _: &EarlyContext<'_>, item: &Item) {
        if matches!(item.kind, ItemKind::Mod(..)) {
            for attribute in &item.attrs {
                self.module_attr_spans.insert(attribute.span);
            }
        }
    }

    /// `check_attribute` runs once per syntactic attribute, after macro
    /// expansion. A `#[cfg_attr(<cfg>, allow(...))]` whose condition
    /// holds is expanded into a synthesised `allow(...)` attribute whose
    /// span covers just the inner `allow(...)` text in the source (no
    /// `#[ ]` wrapper); a `cfg_attr` whose condition fails is dropped
    /// entirely. Both the bare and the `cfg_attr`-derived forms arrive
    /// here as an attribute named `allow`; the two are told apart by
    /// whether the attribute's source snippet still carries its `#[`
    /// delimiter, which decides how the split rewrite is rendered.
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if !attribute.has_name(ALLOW) {
            return;
        }
        if is_from_proc_macro(lint_context, attribute) {
            return;
        }
        if !self.scope_is_eligible(attribute.style, attribute.span) {
            return;
        }
        let Some(ident_span) = attr_path_ident_span(attribute) else {
            return;
        };
        let Some(args) = attribute.meta_item_list() else {
            return;
        };
        self.check_allow(
            lint_context,
            ident_span,
            attribute.span,
            attribute.style,
            &args,
        );
    }
}

/// Where an `allow(...)` invocation lives, which decides how the
/// split-attribute autofix renders its replacement text.
enum Container {
    /// A standalone `#[allow(...)]` / `#![allow(...)]` attribute. The
    /// split rewrite replaces the whole attribute with two attributes.
    Bare { span: Span, style: AttrStyle },
    /// An `allow(...)` nested inside a `cfg_attr` argument list. The
    /// split rewrite replaces just the inner meta item with two
    /// comma-separated `allow(...), expect(...)` invocations, leaving
    /// the `cfg_attr` wrapper and its `cfg` condition untouched.
    CfgAttrInner { span: Span },
}

impl AllowAttributes {
    /// Whether an attribute at this scope is eligible for rewriting.
    /// Crate-root / module-body inner attributes (`#![...]`) and outer
    /// attributes on `mod` items are gated behind `apply_to_outer_scopes`.
    fn scope_is_eligible(&self, style: AttrStyle, span: Span) -> bool {
        if self.apply_to_outer_scopes {
            return true;
        }
        if let AttrStyle::Inner = style {
            return false;
        }
        !self.module_attr_spans.contains(&span)
    }

    /// Apply the rule to a single `allow(...)` invocation.
    ///
    /// `ident_span` covers the `allow` keyword; it anchors the
    /// diagnostic and is the target of the `expect` swap. `attr_span` /
    /// `attr_style` locate the whole attribute, used to classify its
    /// container and render the removal / split suggestions. `args` is
    /// the attribute's argument list.
    fn check_allow(
        &self,
        lint_context: &EarlyContext<'_>,
        ident_span: Span,
        attr_span: Span,
        attr_style: AttrStyle,
        args: &[MetaItemInner],
    ) {
        let mut rewriteable: Vec<String> = Vec::new();
        let mut kept: Vec<String> = Vec::new();
        let mut reason: Option<&MetaItem> = None;

        for arg in args {
            let MetaItemInner::MetaItem(meta) = arg else {
                continue;
            };
            match &meta.kind {
                MetaItemKind::Word => {
                    let name = render_meta_path(meta);
                    if self.is_rewriteable(meta, &name) {
                        rewriteable.push(name);
                    } else {
                        kept.push(name);
                    }
                }
                MetaItemKind::NameValue(_) if meta.has_name(sym::reason) => {
                    reason = Some(meta);
                }
                // `List` and other shapes are not lint names; ignore.
                _ => {}
            }
        }

        // Nothing deterministic to act on — leave the attribute as-is.
        if rewriteable.is_empty() {
            return;
        }

        let reason_snippet = reason.and_then(|meta| snippet_opt(lint_context, meta.span));
        let container = container_of(lint_context, attr_span, attr_style);

        if kept.is_empty() {
            // Every named lint is rewriteable: offer removal of the whole
            // suppression or a one-word `allow` -> `expect` swap.
            self.emit_simple(lint_context, ident_span, container.as_ref());
            return;
        }

        // Mixed: act on the rewriteable lints only — drop them, or split
        // them into a separate `#[expect]`. The textual rewrite needs the
        // source snippet to place the new attribute (bare vs inside a
        // `cfg_attr` arg list) and to copy the `reason` verbatim. Either
        // can be unavailable when a `macro_rules!` attribute is assembled
        // from two source files — e.g. `reason = $reason` with the key in
        // the macro definition and the literal at an `include!`d call
        // site — which leaves a span whose ends sit in different files,
        // and `span_to_snippet` refuses those. Flag the site without a
        // structured suggestion rather than risk a rewrite that drops the
        // reason or injects `#[..]` inside a `cfg_attr`.
        let reason_recoverable = reason.is_none() || reason_snippet.is_some();
        match container {
            Some(container) if reason_recoverable => self.emit_split(
                lint_context,
                ident_span,
                &container,
                &kept,
                &rewriteable,
                reason_snippet.as_deref(),
            ),
            _ => emit_split_without_fix(lint_context, ident_span),
        }
    }

    /// Whether a single lint name is one of the deterministically-firing
    /// kinds this rule rewrites. `name` is the rendered fully-namespaced
    /// form, matched against `exempt_lints`.
    fn is_rewriteable(&self, meta: &MetaItem, name: &str) -> bool {
        if self.exempt_lints.contains(name) {
            return false;
        }
        let segments = &meta.path.segments;
        if segments.len() <= 1 {
            // Bare name: rewriteable only if it is a registered built-in
            // lint. Unknown bare names (and lint groups) are left alone.
            return self.builtin_lints.contains(name);
        }
        // Tool-namespaced. `clippy` and `rustdoc` ship deterministic
        // lints and are always rewriteable — except their lint *groups*
        // (`clippy::pedantic`, `rustdoc::all`, etc.), which fire only if
        // some member fires, exactly the non-determinism the bare-name
        // branch excludes for rustc groups. Every other tool namespace
        // is gated behind `apply_to_tool_namespaces`.
        let tool = segments[0].ident.name.as_str();
        let lint = segments.last().map(|segment| segment.ident.name.as_str());
        match tool {
            "clippy" => lint.is_some_and(|lint| !CLIPPY_LINT_GROUPS.contains(&lint)),
            "rustdoc" => lint.is_some_and(|lint| !RUSTDOC_LINT_GROUPS.contains(&lint)),
            _ => self.apply_to_tool_namespaces,
        }
    }

    /// Emit the lint for a simple `#[allow]` whose every named lint is
    /// rewriteable, offering the two resolutions a deterministic
    /// suppression has. The lint might still fire here (so `#[expect]`
    /// keeps the suppression and makes it self-clean) or might be long
    /// dead (so the suppression should go) — the rule can't tell which,
    /// so both are `MaybeIncorrect` hints for the author to choose.
    fn emit_simple(
        &self,
        lint_context: &EarlyContext<'_>,
        ident_span: Span,
        container: Option<&Container>,
    ) {
        // Removal is only offered for a bare attribute; the inner `allow`
        // of a `cfg_attr` can't be deleted without dropping its `cfg`
        // wrapper. When it isn't offered, the `#[expect]` suggestion drops
        // its "otherwise" lead-in, which would otherwise dangle with no
        // alternative to contrast against.
        let removable = matches!(container, Some(Container::Bare { .. }));
        let replace_help = if removable {
            "otherwise replace `allow` with `expect`, which warns once the lint stops firing"
        } else {
            "replace `allow` with `expect`, which warns once the lint stops firing"
        };
        span_lint_and_then(
            lint_context,
            ALLOW_ATTRIBUTES,
            ident_span,
            "this `#[allow]` stays silent even after its lint stops firing",
            |diag| {
                // Resolution 1: remove the suppression — correct when the
                // lint can no longer fire here. Only a bare attribute can
                // be deleted outright; the inner `allow` of a `cfg_attr`
                // can't go without dropping its `cfg` wrapper, so removal
                // is left to the author in that case.
                if let Some(Container::Bare { span, .. }) = container {
                    diag.span_suggestion(
                        *span,
                        "remove the suppression if its lint can no longer fire here",
                        String::new(),
                        Applicability::MaybeIncorrect,
                    );
                }
                // Resolution 2: keep the suppression but make it report
                // once the lint stops firing.
                diag.span_suggestion(
                    ident_span,
                    replace_help,
                    "expect".to_owned(),
                    Applicability::MaybeIncorrect,
                );
            },
        );
    }

    /// Emit the lint for a mixed `#[allow]`, acting on the rewriteable
    /// lints only: drop them, or split them into a separate `#[expect]`.
    /// The kept (non-rewriteable) names stay under `#[allow]` either way.
    /// Like [`Self::emit_simple`], both are `MaybeIncorrect` hints.
    fn emit_split(
        &self,
        lint_context: &EarlyContext<'_>,
        ident_span: Span,
        container: &Container,
        kept: &[String],
        rewriteable: &[String],
        reason: Option<&str>,
    ) {
        let span = container.span();
        let drop_rewriteable = render_attrs(lint_context, container, &[("allow", kept)], reason);
        let split = render_attrs(
            lint_context,
            container,
            &[("allow", kept), ("expect", rewriteable)],
            reason,
        );
        span_lint_and_then(
            lint_context,
            ALLOW_ATTRIBUTES,
            ident_span,
            "some lints in this `#[allow]` stay silent even after they stop firing",
            |diag| {
                diag.span_suggestion(
                    span,
                    "drop them from the `#[allow]` if they can no longer fire here",
                    drop_rewriteable,
                    Applicability::MaybeIncorrect,
                );
                diag.span_suggestion(
                    span,
                    "otherwise split them into a separate `#[expect]` that self-cleans",
                    split,
                    Applicability::MaybeIncorrect,
                );
            },
        );
    }
}

impl Container {
    /// The span the suggestions replace — the whole bare attribute, or
    /// the inner `allow(...)` item of a `cfg_attr`.
    fn span(&self) -> Span {
        match *self {
            Container::Bare { span, .. } | Container::CfgAttrInner { span } => span,
        }
    }
}

/// Classify where an `allow` attribute lives, for the split rewrite. A
/// real bare `#[allow(...)]` / `#![allow(...)]` keeps its `#` delimiter
/// in the source snippet; a `cfg_attr`-synthesised `allow(...)` does
/// not, so its split must stay inside the `cfg_attr` argument list
/// rather than emit standalone `#[ ]` attributes. Returns `None` when
/// the snippet is unavailable, so the caller declines the autofix
/// instead of guessing the wrapper.
fn container_of(
    lint_context: &EarlyContext<'_>,
    span: Span,
    style: AttrStyle,
) -> Option<Container> {
    let snippet = snippet_opt(lint_context, span)?;
    Some(if snippet.trim_start().starts_with('#') {
        Container::Bare { span, style }
    } else {
        Container::CfgAttrInner { span }
    })
}

/// Flag a mixed `#[allow]` without structured suggestions, used when the
/// source text needed to render them is unavailable. Describes both
/// resolutions in prose instead.
fn emit_split_without_fix(lint_context: &EarlyContext<'_>, ident_span: Span) {
    span_lint_and_help(
        lint_context,
        ALLOW_ATTRIBUTES,
        ident_span,
        "some lints in this `#[allow]` stay silent even after they stop firing",
        None,
        "drop them from the `#[allow]` if they can no longer fire here, or split them \
         out into a separate `#[expect]` that self-cleans",
    );
}

/// Render the replacement text for a list of lint-control invocations
/// (`("allow", names)`, `("expect", names)`) in the form `container`
/// expects: standalone `#[..]` attributes for a bare `#[allow]`, or a
/// comma-joined `allow(..), expect(..)` for the inner item of a
/// `cfg_attr`. Groups with no names are skipped, so passing only a
/// non-empty `("allow", kept)` renders the attribute with the
/// rewriteable lints dropped.
fn render_attrs(
    lint_context: &EarlyContext<'_>,
    container: &Container,
    groups: &[(&str, &[String])],
    reason: Option<&str>,
) -> String {
    let bodies = groups
        .iter()
        .filter(|(_, names)| !names.is_empty())
        .map(|(keyword, names)| render_invocation(keyword, names, reason));
    match container {
        Container::CfgAttrInner { .. } => bodies.collect::<Vec<_>>().join(", "),
        Container::Bare { span, style } => {
            let hash = match style {
                AttrStyle::Inner => "#!",
                AttrStyle::Outer => "#",
            };
            let pad = " ".repeat(indent_of(lint_context, *span).unwrap_or(0));
            bodies
                .map(|body| format!("{hash}[{body}]"))
                .collect::<Vec<_>>()
                .join(&format!("\n{pad}"))
        }
    }
}

/// Render an `allow(...)` / `expect(...)` invocation from a list of
/// lint names and an optional verbatim `reason = "..."` snippet.
fn render_invocation(keyword: &str, names: &[String], reason: Option<&str>) -> String {
    let parts: Vec<&str> = names.iter().map(String::as_str).chain(reason).collect();
    format!("{keyword}({})", parts.join(", "))
}

/// Span of the `allow` identifier in an attribute's path.
fn attr_path_ident_span(attribute: &Attribute) -> Option<Span> {
    let item = attribute.get_normal_item();
    item.path.segments.first().map(|segment| segment.ident.span)
}
