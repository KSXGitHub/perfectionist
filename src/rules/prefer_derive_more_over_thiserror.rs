use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::{
    AttrVec, Attribute, Crate, EnumDef, Item, ItemKind, ModKind, UseTree, UseTreeKind, VariantData,
};
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol, kw, sym};

use crate::common::{DefaultState, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// Flags every use of [`thiserror`](https://docs.rs/thiserror) in
    /// the consumer crate. Three syntactic shapes trigger the lint:
    ///
    /// 1. `#[derive(thiserror::Error)]` — or `#[derive(Error)]` when a
    ///    sibling `use thiserror::Error;` brings the derive macro into
    ///    scope under any local name.
    /// 2. `#[error(...)]` attributes attached to an item that this
    ///    rule has already classified as thiserror-derived.
    /// 3. `use thiserror::*`, `use thiserror::Error`,
    ///    `use thiserror::Error as MyError;`, and similar imports
    ///    that bring `thiserror`'s items into scope.
    ///
    /// The lint is detection-only: it emits a help-style diagnostic
    /// pointing at the offending site and suggests migrating to
    /// `#[derive(derive_more::Display, derive_more::Error)]`. There is
    /// no autofix — the migration involves a mix of derive-list edits,
    /// format-string positional translation (`thiserror`'s `{0}` ↔
    /// `derive_more`'s `{_0}`), attribute renames (`#[error(...)]` ↔
    /// `#[display(...)]`), and edge cases (`#[error(transparent)]`,
    /// `#[backtrace]`) whose mechanical rewrite is too risky to apply
    /// without review.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. The
    /// catalogue picks `derive_more` for error formatting and source
    /// chaining. Mixing in `thiserror` fragments the attribute
    /// vocabulary across the codebase and adds a second derive crate
    /// that has no functional capability `derive_more` lacks. A
    /// project that wants the choice the other way around can disable
    /// this rule.
    ///
    /// ### Example
    /// ```rust,ignore
    /// use thiserror::Error;
    ///
    /// #[derive(Debug, Error)]
    /// pub enum MyError {
    ///     #[error("missing field {0}")]
    ///     MissingField(String),
    /// }
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// use derive_more::{Display, Error};
    ///
    /// #[derive(Debug, Display, Error)]
    /// pub enum MyError {
    ///     #[display("missing field {_0}")]
    ///     MissingField(String),
    /// }
    /// ```
    pub perfectionist::PREFER_DERIVE_MORE_OVER_THISERROR,
    Warn,
    "error type derived through `thiserror`; this catalogue prefers `derive_more::{Display, Error}`",
    report_in_external_macro: false
}

/// Active by default. Read by `register_pass` below; gen-docs picks
/// the constant up via syn to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

const CONFIG_KEY: &str = "perfectionist::prefer_derive_more_over_thiserror";

/// Recognised `thiserror` derive paths. The default covers the
/// canonical crate; a project that re-publishes the derive under a
/// different crate name can extend the list.
const DEFAULT_THISERROR_PATHS: &[&str] = &["thiserror::Error"];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Paths whose presence in a `#[derive(...)]` list (or whose
    /// crate's presence in a `use` statement) flags the site. Each
    /// entry is a `::`-separated path string. Replaces the default
    /// `["thiserror::Error"]` when supplied.
    thiserror_paths: Option<Vec<String>>,
}

pub struct PreferDeriveMoreOverThiserror {
    /// Configured paths split into segment lists (e.g.
    /// `[[thiserror, Error]]`).
    thiserror_paths: Vec<Vec<Symbol>>,
    /// First segments of every configured path — the crate names a
    /// `use` statement must start with to be flagged.
    thiserror_crates: BTreeSet<Symbol>,
    /// Identifiers that, anywhere in the crate, are bound by a
    /// `use thiserror::...` (or aliased form). A bare `#[derive(X)]`
    /// where `X` is in this set is treated as thiserror-derived.
    /// Populated by [`Self::collect_aliases`] from the
    /// [`EarlyLintPass::check_crate`] hook, before any
    /// [`EarlyLintPass::check_item`] callback runs.
    aliases: BTreeSet<Symbol>,
}

impl PreferDeriveMoreOverThiserror {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let configured = config.thiserror_paths.unwrap_or_else(|| {
            DEFAULT_THISERROR_PATHS
                .iter()
                .map(|path| (*path).to_owned())
                .collect()
        });
        let thiserror_paths: Vec<Vec<Symbol>> = configured
            .iter()
            .map(|path| {
                path.split("::")
                    .filter(|segment| !segment.is_empty())
                    .map(Symbol::intern)
                    .collect()
            })
            .filter(|segments: &Vec<Symbol>| !segments.is_empty())
            .collect();
        let thiserror_crates = thiserror_paths
            .iter()
            .filter_map(|segments| segments.first().copied())
            .collect();
        Self {
            thiserror_paths,
            thiserror_crates,
            aliases: BTreeSet::new(),
        }
    }
}

impl_lint_pass!(PreferDeriveMoreOverThiserror => [PREFER_DERIVE_MORE_OVER_THISERROR]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[PREFER_DERIVE_MORE_OVER_THISERROR]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("prefer_derive_more_over_thiserror", DEFAULT_STATE)
    {
        return;
    }
    // Pre-expansion: derives are consumed during macro expansion, so a
    // regular (post-expansion) pass no longer sees the
    // `#[derive(...)]` attribute by the time the rule looks for it.
    // The sibling `perfectionist::derive_ordering` rule uses the same
    // hook for the same reason.
    lint_store.register_pre_expansion_pass(|| Box::new(PreferDeriveMoreOverThiserror::new()));
}

impl EarlyLintPass for PreferDeriveMoreOverThiserror {
    fn check_crate(&mut self, _cx: &EarlyContext<'_>, krate: &Crate) {
        for item in &krate.items {
            self.collect_aliases(item);
        }
    }

    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Use(use_tree) => self.check_use(cx, item, use_tree),
            ItemKind::Struct(_, _, data) => self.check_struct(cx, &item.attrs, data),
            ItemKind::Enum(_, _, def) => self.check_enum(cx, &item.attrs, def),
            _ => {}
        }
    }
}

impl PreferDeriveMoreOverThiserror {
    /// Recurse through inline modules to find `use` statements that
    /// pull a configured `thiserror_paths` entry into scope. Each
    /// such import contributes its local name (rename, or the path's
    /// last segment) to [`Self::aliases`].
    fn collect_aliases(&mut self, item: &Item) {
        match &item.kind {
            ItemKind::Use(use_tree) => {
                self.walk_use_tree(use_tree, &[]);
            }
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, _)) => {
                for nested in items {
                    self.collect_aliases(nested);
                }
            }
            _ => {}
        }
    }

    fn walk_use_tree(&mut self, tree: &UseTree, parent: &[Symbol]) {
        let mut path: Vec<Symbol> = parent.to_vec();
        for segment in &tree.prefix.segments {
            if segment.ident.name == kw::PathRoot {
                continue;
            }
            path.push(segment.ident.name);
        }
        match &tree.kind {
            UseTreeKind::Simple(rename) => {
                if !path_matches_thiserror(&self.thiserror_paths, &path) {
                    return;
                }
                let local = rename
                    .map(|ident| ident.name)
                    .or_else(|| path.last().copied());
                if let Some(local) = local {
                    self.aliases.insert(local);
                }
            }
            UseTreeKind::Glob(_) => {
                for cfg in &self.thiserror_paths {
                    if cfg.len() > path.len()
                        && cfg.starts_with(&path)
                        && let Some(&last) = cfg.last()
                    {
                        self.aliases.insert(last);
                    }
                }
            }
            UseTreeKind::Nested { items, .. } => {
                for (nested, _) in items {
                    self.walk_use_tree(nested, &path);
                }
            }
        }
    }

    fn check_struct(&self, cx: &EarlyContext<'_>, attrs: &AttrVec, data: &VariantData) {
        if !self.check_derive_list(cx, attrs) {
            return;
        }
        flag_error_attrs(cx, attrs);
        flag_variant_data_error_attrs(cx, data);
    }

    fn check_enum(&self, cx: &EarlyContext<'_>, attrs: &AttrVec, def: &EnumDef) {
        if !self.check_derive_list(cx, attrs) {
            return;
        }
        flag_error_attrs(cx, attrs);
        flag_enum_error_attrs(cx, def);
    }

    fn check_use(&self, cx: &EarlyContext<'_>, item: &Item, use_tree: &UseTree) {
        let first = use_tree
            .prefix
            .segments
            .iter()
            .map(|segment| segment.ident.name)
            .find(|name| *name != kw::PathRoot);
        let Some(first) = first else {
            return;
        };
        if !self.thiserror_crates.contains(&first) {
            return;
        }
        emit_use(cx, item.span);
    }

    /// Walk a struct or enum's outer attributes and emit a
    /// diagnostic on every derive entry that resolves to a
    /// configured thiserror path. Returns `true` when at least one
    /// entry matched, signalling that the caller should also flag
    /// `#[error(...)]` attributes elsewhere on the item.
    fn check_derive_list(&self, cx: &EarlyContext<'_>, attrs: &AttrVec) -> bool {
        let mut thiserror_derived = false;
        for attr in attrs {
            if !attr.has_name(sym::derive) {
                continue;
            }
            let Some(entries) = attr.meta_item_list() else {
                continue;
            };
            for entry in &entries {
                let Some(meta) = entry.meta_item() else {
                    continue;
                };
                let segments: Vec<Symbol> = meta
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.name)
                    .filter(|name| *name != kw::PathRoot)
                    .collect();
                if !self.is_thiserror_derive(&segments) {
                    continue;
                }
                thiserror_derived = true;
                emit_derive(cx, entry.span());
            }
        }
        thiserror_derived
    }

    fn is_thiserror_derive(&self, segments: &[Symbol]) -> bool {
        if path_matches_thiserror(&self.thiserror_paths, segments) {
            return true;
        }
        // Single-segment derive entry (`#[derive(Error)]`) matches
        // when the crate has a sibling `use thiserror::Error;` (or
        // aliased / glob form) somewhere in scope.
        matches!(segments, [name] if self.aliases.contains(name))
    }
}

fn path_matches_thiserror(configured: &[Vec<Symbol>], path: &[Symbol]) -> bool {
    configured.iter().any(|cfg| cfg.as_slice() == path)
}

fn flag_error_attrs(cx: &EarlyContext<'_>, attrs: &[Attribute]) {
    // `Symbol::intern("error")` rather than a `kw::` / `sym::`
    // constant because `error` is not a pre-interned compiler
    // symbol. The intern lookup happens once per attribute on
    // thiserror-derived items, which is cheap relative to the
    // surrounding diagnostic emission.
    let error = Symbol::intern("error");
    for attr in attrs {
        if attr.has_name(error) {
            emit_error_attr(cx, attr.span);
        }
    }
}

fn flag_variant_data_error_attrs(cx: &EarlyContext<'_>, data: &VariantData) {
    for field in data.fields() {
        flag_error_attrs(cx, &field.attrs);
    }
}

fn flag_enum_error_attrs(cx: &EarlyContext<'_>, def: &EnumDef) {
    for variant in &def.variants {
        flag_error_attrs(cx, &variant.attrs);
        flag_variant_data_error_attrs(cx, &variant.data);
    }
}

fn emit_use(cx: &EarlyContext<'_>, span: Span) {
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "`use` of `thiserror`; this catalogue prefers `derive_more::{Display, Error}`",
        None,
        "drop the import and migrate the error type to `#[derive(derive_more::Display, \
         derive_more::Error)]`",
    );
}

fn emit_derive(cx: &EarlyContext<'_>, span: Span) {
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "error type derived through `thiserror`; this catalogue prefers \
         `derive_more::{Display, Error}`",
        None,
        "replace the derive list and migrate the `#[error(...)]` attributes to `#[display(...)]`",
    );
}

fn emit_error_attr(cx: &EarlyContext<'_>, span: Span) {
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "`#[error(...)]` attribute belongs to `thiserror`'s namespace; this catalogue prefers \
         `derive_more::Display`",
        None,
        "rename to `#[display(...)]` and translate positional placeholders (`{0}` -> `{_0}`)",
    );
}
