use std::collections::{BTreeMap, BTreeSet};

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{
    AttrVec, Attribute, Crate, EnumDef, Item, ItemKind, MetaItemInner, MetaItemKind, UseTree,
    UseTreeKind, VariantData,
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
    /// 1. **Derives.** `#[derive(thiserror::Error)]` directly, or
    ///    `#[derive(Error)]` / `#[derive(te::Error)]` when a sibling
    ///    `use thiserror::Error;` / `use thiserror as te;` brings the
    ///    derive macro into scope under any local name, anywhere in
    ///    the crate. `#[cfg_attr(_, derive(thiserror::Error))]` is
    ///    unwrapped (including nested `cfg_attr`).
    /// 2. **Attributes.** `#[error(...)]` attributes attached to an
    ///    item the rule has already classified as thiserror-derived,
    ///    on the item, its enum variants, or its fields.
    ///    `#[cfg_attr(_, error(...))]` is unwrapped symmetrically
    ///    with the derive side.
    /// 3. **Imports.** Every `use` or `extern crate` statement that
    ///    brings a configured `thiserror` path into scope:
    ///    `use thiserror::*`, `use thiserror::Error`,
    ///    `use thiserror::Error as MyError;`,
    ///    `use thiserror::{self as te};`, `use thiserror as te;`,
    ///    `extern crate thiserror;`, `extern crate thiserror as te;`,
    ///    the braced top-level form `use {thiserror::Error, ...};`,
    ///    and `pub use` re-exports.
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
    /// Because the pass runs pre-expansion and does not consult
    /// name resolution, alias collection is crate-wide rather than
    /// per-module: a `use thiserror::Error;` anywhere in the crate
    /// makes the bare `#[derive(Error)]` short-hand resolve as
    /// thiserror everywhere. In practice that overlap is rare and
    /// the rule treats it as acceptable false-positive surface; a
    /// project that hits it can suppress individual sites with
    /// `#[allow(perfectionist::prefer_derive_more_over_thiserror)]`.
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
    /// `["thiserror::Error"]` when supplied; the empty list `[]`
    /// disables the rule.
    thiserror_paths: Option<Vec<String>>,
}

pub struct PreferDeriveMoreOverThiserror {
    /// Configured paths split into segment lists (e.g.
    /// `[[thiserror, Error]]`).
    thiserror_paths: Vec<Vec<Symbol>>,
    /// First segments of every configured path — the crate names a
    /// `use` statement must start with to be flagged.
    thiserror_crates: BTreeSet<Symbol>,
    /// Identifiers that, anywhere in the crate, name a configured
    /// path's terminal item. A bare `#[derive(X)]` where `X` is in
    /// this set is treated as thiserror-derived. Populated by the
    /// [`AliasCollector`] visitor from the [`EarlyLintPass::check_crate`]
    /// hook, before any [`EarlyLintPass::check_item`] callback runs.
    aliases: BTreeSet<Symbol>,
    /// Local names that alias a configured thiserror *crate*.
    /// Populated from `use thiserror as te;` and
    /// `extern crate thiserror as te;`. The value is the original
    /// crate name (e.g. `thiserror`) so that a derive path
    /// `[te, Error]` can be expanded to `[thiserror, Error]` and
    /// matched against [`Self::thiserror_paths`].
    crate_aliases: BTreeMap<Symbol, Symbol>,
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
            crate_aliases: BTreeMap::new(),
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
        // `thiserror_paths` is empty when the user opts out via
        // `thiserror_paths = []` in `dylint.toml`. Skip the
        // crate-wide alias scan and short-circuit every per-item
        // check below — there is nothing to match against.
        if self.thiserror_paths.is_empty() {
            return;
        }
        // Reset state. dylint currently constructs a fresh pass per
        // crate, so the clear is defensive; a future host that reuses
        // instances would otherwise see aliases leak between crates.
        self.aliases.clear();
        self.crate_aliases.clear();
        // Two-pass alias collection: the leaf-alias pass needs to
        // expand a crate-aliased first segment (`use te::Error;` after
        // `use thiserror as te;`) through `crate_aliases`, but the
        // visitor walks items in source order, and the two `use`
        // statements may appear in either order. The first pass
        // therefore fills `crate_aliases` only, so the second pass
        // always sees a complete map regardless of declaration order.
        // Both passes use the same AST visitor (rather than a
        // hand-rolled mod-only walk) so that `use thiserror::...`
        // inside fn bodies, blocks, `impl`s, and trait items still
        // feeds the alias set.
        for phase in [CollectPhase::CrateAliases, CollectPhase::LeafAliases] {
            let mut collector = AliasCollector { rule: self, phase };
            visit::walk_crate(&mut collector, krate);
        }
    }

    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if self.thiserror_paths.is_empty() {
            return;
        }
        match &item.kind {
            ItemKind::Use(use_tree) => self.check_use(cx, item, use_tree),
            ItemKind::ExternCrate(orig, ident) => {
                // `extern crate thiserror;` — `orig` is `None`,
                // `ident.name` is the crate name. `extern crate
                // thiserror as te;` — `orig` is `Some(thiserror)`,
                // `ident.name` is `te`. The alias side is recorded by
                // `AliasCollector`; emit the `use`-shaped diagnostic
                // on the item span here.
                let original = orig.unwrap_or(ident.name);
                if self.thiserror_crates.contains(&original) {
                    emit_use(cx, item.span);
                }
            }
            ItemKind::Struct(_, _, data) => self.check_struct(cx, &item.attrs, data),
            ItemKind::Enum(_, _, def) => self.check_enum(cx, &item.attrs, def),
            _ => {}
        }
    }
}

/// Phase tag for the two-pass alias collection driven by
/// [`AliasCollector`]. The first pass fills `crate_aliases` from
/// every `use thiserror as te;` / `extern crate thiserror as te;`
/// item; the second pass fills `aliases`, expanding crate-aliased
/// first segments (`use te::Error;`) through `crate_aliases`.
#[derive(Clone, Copy)]
enum CollectPhase {
    CrateAliases,
    LeafAliases,
}

/// AST visitor used by [`EarlyLintPass::check_crate`] to populate
/// the rule's alias maps. Walking the AST through the visitor (rather
/// than a hand-rolled `ItemKind::Mod`-only recursion) ensures that
/// `use thiserror::...` and `extern crate thiserror as ...;` items
/// nested inside function bodies, blocks, `impl`s, or trait items are
/// recorded — the rule's `check_item` callback still visits those
/// inner items individually for diagnostic emission, but the alias
/// scan must happen up front so that bare `#[derive(Error)]` in any
/// scope can be resolved against a sibling `use thiserror::Error;`
/// in any other scope.
struct AliasCollector<'a> {
    rule: &'a mut PreferDeriveMoreOverThiserror,
    phase: CollectPhase,
}

impl<'ast> Visitor<'ast> for AliasCollector<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        match &item.kind {
            ItemKind::Use(use_tree) => {
                walk_use_tree_for_aliases(self.rule, use_tree, &[], self.phase);
            }
            ItemKind::ExternCrate(orig, ident) => {
                // `extern crate thiserror;` — `orig` is `None`,
                // `ident` is `thiserror`. `extern crate thiserror as te;`
                // — `orig` is `Some(thiserror)`, `ident` is `te`. Only
                // relevant during the crate-alias pass; the leaf-alias
                // pass relies on the already-populated map without
                // re-inserting.
                if matches!(self.phase, CollectPhase::CrateAliases) {
                    let original = orig.unwrap_or(ident.name);
                    if self.rule.thiserror_crates.contains(&original) {
                        self.rule.crate_aliases.insert(ident.name, original);
                    }
                }
            }
            _ => {}
        }
        visit::walk_item(self, item);
    }
}

fn walk_use_tree_for_aliases(
    rule: &mut PreferDeriveMoreOverThiserror,
    tree: &UseTree,
    parent: &[Symbol],
    phase: CollectPhase,
) {
    let mut path: Vec<Symbol> = parent.to_vec();
    for (idx, segment) in tree.prefix.segments.iter().enumerate() {
        // `kw::PathRoot` is the synthetic leading-`::` segment; skip
        // it so `use ::thiserror::Error;` and `use thiserror::Error;`
        // produce the same `path`.
        if segment.ident.name == kw::PathRoot {
            continue;
        }
        // `kw::SelfLower` at position 0 of a *nested child's* prefix
        // (`use thiserror::{self};` / `use thiserror::{self as te};`)
        // names the parent itself — semantically equivalent to `use
        // thiserror;` / `use thiserror as te;`. Drop the segment so
        // the nested form joins the parent path and falls into the
        // single-segment crate-alias branch below.
        //
        // At the top level (`use self::Error;`) `self` is a real
        // module reference and must NOT be filtered, since
        // `self::Error` resolves through the current module rather
        // than the crate root. Gating on `!parent.is_empty()`
        // distinguishes the two positions.
        if !parent.is_empty() && idx == 0 && segment.ident.name == kw::SelfLower {
            continue;
        }
        path.push(segment.ident.name);
    }
    match &tree.kind {
        UseTreeKind::Simple(rename) => match phase {
            CollectPhase::CrateAliases => {
                // `use thiserror as te;` / `use thiserror;` —
                // single-segment path naming a thiserror crate root.
                // Record the local name as a crate-level alias so a
                // later `te::Error` derive (or `use te::Error;`) can
                // be expanded.
                if path.len() == 1 && rule.thiserror_crates.contains(&path[0]) {
                    let local = rename.map(|ident| ident.name).unwrap_or(path[0]);
                    rule.crate_aliases.insert(local, path[0]);
                }
            }
            CollectPhase::LeafAliases => {
                // `use thiserror::Error;` / `use thiserror::Error as
                // X;` — direct leaf match. `use te::Error;` after a
                // sibling `use thiserror as te;` — same shape after
                // first-segment expansion through `crate_aliases`.
                let matches_direct = path_matches_thiserror(&rule.thiserror_paths, &path);
                let matches_via_alias = !matches_direct
                    && match path.split_first() {
                        Some((first, rest)) => rule
                            .crate_aliases
                            .get(first)
                            .map(|&expanded| {
                                let mut expanded_path = Vec::with_capacity(path.len());
                                expanded_path.push(expanded);
                                expanded_path.extend_from_slice(rest);
                                path_matches_thiserror(&rule.thiserror_paths, &expanded_path)
                            })
                            .unwrap_or(false),
                        None => false,
                    };
                if matches_direct || matches_via_alias {
                    let local = rename
                        .map(|ident| ident.name)
                        .or_else(|| path.last().copied());
                    if let Some(local) = local {
                        rule.aliases.insert(local);
                    }
                }
            }
        },
        UseTreeKind::Glob(_) => {
            if matches!(phase, CollectPhase::CrateAliases) {
                // Globs don't introduce crate aliases.
                return;
            }
            // `use a::b::*;` brings the *direct* children of `a::b`
            // into scope, so the glob exposes the configured leaf
            // only when the configured path is exactly one segment
            // deeper than the glob's path. For default config
            // `[[thiserror, Error]]` plus `use thiserror::*;` the
            // check is `2 == 1 + 1` and `Error` is inserted; for a
            // hypothetical multi-segment config like
            // `["foo::bar::Error"]` plus `use foo::*;` it is
            // `3 == 1 + 1 ⇒ false` and the rule does not falsely
            // claim `Error` is in scope.
            //
            // The glob's path is also expanded through `crate_aliases`
            // so `use te::*;` after `use thiserror as te;` behaves
            // like `use thiserror::*;`.
            let expanded_path = match path.split_first() {
                Some((first, rest)) => match rule.crate_aliases.get(first) {
                    Some(&expanded_first) => {
                        let mut out = Vec::with_capacity(path.len());
                        out.push(expanded_first);
                        out.extend_from_slice(rest);
                        out
                    }
                    None => path.clone(),
                },
                None => path.clone(),
            };
            for cfg in &rule.thiserror_paths {
                if cfg.len() == expanded_path.len() + 1
                    && cfg.starts_with(&expanded_path)
                    && let Some(&last) = cfg.last()
                {
                    rule.aliases.insert(last);
                }
            }
        }
        UseTreeKind::Nested { items, .. } => {
            for (nested, _) in items {
                walk_use_tree_for_aliases(rule, nested, &path, phase);
            }
        }
    }
}

impl PreferDeriveMoreOverThiserror {
    fn check_struct(&self, cx: &EarlyContext<'_>, attrs: &AttrVec, data: &VariantData) {
        if !self.check_derive_attrs(cx, attrs) {
            return;
        }
        flag_error_attrs(cx, attrs);
        flag_variant_data_error_attrs(cx, data);
    }

    fn check_enum(&self, cx: &EarlyContext<'_>, attrs: &AttrVec, def: &EnumDef) {
        if !self.check_derive_attrs(cx, attrs) {
            return;
        }
        flag_error_attrs(cx, attrs);
        flag_enum_error_attrs(cx, def);
    }

    /// Emit on every `use` statement whose tree touches a configured
    /// thiserror crate at any depth. Handles the three common shapes
    /// in one pass:
    ///
    /// - `use thiserror::Error;` — outer prefix's first segment is
    ///   `thiserror`, matches immediately.
    /// - `use thiserror::{Error, ErrorKind};` — same as above; the
    ///   nested children inherit the outer prefix, so we don't have
    ///   to walk them.
    /// - `use {thiserror::Error, std::io};` — top-level braced form
    ///   with an empty outer prefix; recurse into the children and
    ///   flag if any child's prefix names a thiserror crate.
    fn check_use(&self, cx: &EarlyContext<'_>, item: &Item, use_tree: &UseTree) {
        if self.use_tree_references_thiserror(use_tree) {
            emit_use(cx, item.span);
        }
    }

    fn use_tree_references_thiserror(&self, tree: &UseTree) -> bool {
        let first = tree
            .prefix
            .segments
            .iter()
            .map(|segment| segment.ident.name)
            .find(|name| *name != kw::PathRoot);
        if let Some(name) = first {
            // Direct match — `use thiserror::...`. Also match through
            // `crate_aliases` so that `use te::Error;` after a sibling
            // `use thiserror as te;` is flagged on its own line, not
            // just on the rename.
            return self.thiserror_crates.contains(&name) || self.crate_aliases.contains_key(&name);
        }
        // Empty outer prefix — recurse into nested children. (A
        // glob with an empty prefix is `use *;` and not valid Rust;
        // a simple with an empty prefix is also invalid. Only the
        // nested form needs handling here.)
        if let UseTreeKind::Nested { items, .. } = &tree.kind {
            return items
                .iter()
                .any(|(nested, _)| self.use_tree_references_thiserror(nested));
        }
        false
    }

    /// Walk a struct or enum's outer attributes and emit a
    /// diagnostic on every derive entry that resolves to a
    /// configured thiserror path. Recurses through `#[cfg_attr]`
    /// wrappers so that conditional derives are not silently
    /// missed. Returns `true` when at least one entry matched,
    /// signalling that the caller should also flag `#[error(...)]`
    /// attributes elsewhere on the item.
    fn check_derive_attrs(&self, cx: &EarlyContext<'_>, attrs: &AttrVec) -> bool {
        let mut thiserror_derived = false;
        for attr in attrs {
            if attr.has_name(sym::derive)
                && let Some(entries) = attr.meta_item_list()
                && self.check_derive_entries(cx, &entries)
            {
                thiserror_derived = true;
            } else if attr.has_name(sym::cfg_attr)
                && let Some(args) = attr.meta_item_list()
                && self.check_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]))
            {
                thiserror_derived = true;
            }
        }
        thiserror_derived
    }

    /// Inspect the attribute-list payload of a `#[cfg_attr(pred, ...)]`
    /// — the slice after the predicate. Each payload entry is itself
    /// an attribute shape; recognise nested `derive(...)` and
    /// further nested `cfg_attr(...)` and recurse.
    fn check_cfg_attr_payload(&self, cx: &EarlyContext<'_>, items: &[MetaItemInner]) -> bool {
        let mut found = false;
        for item in items {
            let Some(meta) = item.meta_item() else {
                continue;
            };
            if meta.has_name(sym::derive)
                && let MetaItemKind::List(entries) = &meta.kind
                && self.check_derive_entries(cx, entries)
            {
                found = true;
            } else if meta.has_name(sym::cfg_attr)
                && let MetaItemKind::List(args) = &meta.kind
                && self.check_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]))
            {
                found = true;
            }
        }
        found
    }

    fn check_derive_entries(&self, cx: &EarlyContext<'_>, entries: &[MetaItemInner]) -> bool {
        let mut thiserror_derived = false;
        for entry in entries {
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
            if self.is_thiserror_derive(&segments) {
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
        if let [name] = segments
            && self.aliases.contains(name)
        {
            return true;
        }
        // Crate-aliased derive entry (`#[derive(te::Error)]` paired
        // with `use thiserror as te;`): expand the first segment to
        // the original crate name and re-match against configured
        // paths.
        if let [first, rest @ ..] = segments
            && let Some(&expanded_first) = self.crate_aliases.get(first)
        {
            let mut expanded = Vec::with_capacity(segments.len());
            expanded.push(expanded_first);
            expanded.extend_from_slice(rest);
            return path_matches_thiserror(&self.thiserror_paths, &expanded);
        }
        false
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
        } else if attr.has_name(sym::cfg_attr)
            && let Some(args) = attr.meta_item_list()
        {
            // Symmetric to `check_derive_attrs`: unwrap
            // `#[cfg_attr(pred, error(...))]` so conditional
            // `#[error(...)]` attributes are caught alongside their
            // sibling `#[cfg_attr(pred, derive(thiserror::Error))]`.
            flag_error_in_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]), error);
        }
    }
}

fn flag_error_in_cfg_attr_payload(cx: &EarlyContext<'_>, items: &[MetaItemInner], error: Symbol) {
    for item in items {
        let Some(meta) = item.meta_item() else {
            continue;
        };
        if meta.has_name(error) {
            emit_error_attr(cx, meta.span);
        } else if meta.has_name(sym::cfg_attr)
            && let MetaItemKind::List(args) = &meta.kind
        {
            flag_error_in_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]), error);
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
    // The helper is shared between `use thiserror::...`, `extern
    // crate thiserror`, and the braced top-level `use {...}` form,
    // so the message uses the neutral word "import" rather than
    // saying "`use`".
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "`thiserror` import; this catalogue prefers `derive_more::{Display, Error}`",
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
