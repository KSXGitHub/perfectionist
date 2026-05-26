//! `perfectionist::import_granularity` — enforce a project-wide
//! import-granularity style (`crate` / `module` / `item`).
//!
//! The flattening of `use` trees lives in [`model`], the per-style
//! compliance predicates in [`check`], and the canonical rendering in
//! [`render`]. This file owns the lint declaration, the configuration,
//! the early pass that walks module bodies, and the diagnostic that
//! replaces a non-conforming group of `use` statements.

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::indent_of;
use rustc_ast::{AttrKind, Attribute, Crate, Item, ItemKind, ModKind, Visibility, VisibilityKind};
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, sym};

mod check;
mod config;
mod model;
mod render;

use check::is_compliant;
use config::{Config, Style};
use model::{Leaf, StmtInfo, stmt_info};

use crate::common::{DefaultState, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// Enforces a single project-wide import-granularity style, chosen
    /// via `style`:
    /// - `crate` — one `use` per crate root, with every shared prefix
    ///   collapsed into nested braces
    ///   (`use std::{collections::HashMap, io::Read};`).
    /// - `module` (default) — one `use` per leaf module; items from the
    ///   same module are merged into one braced list while sibling
    ///   modules sit on their own lines
    ///   (`use std::collections::{BTreeMap, HashMap};`).
    /// - `item` — one `use` per leaf path
    ///   (`use std::collections::BTreeMap;`).
    ///
    /// The names map one-to-one onto rustfmt's unstable
    /// `imports_granularity` (`Crate` / `Module` / `Item`). Only `use`
    /// statements that sit next to each other in a module body, share a
    /// visibility, and carry matching attributes are merged; the three
    /// `respect_*` knobs tighten or loosen that grouping.
    ///
    /// Globs (`use foo::*`) are governed by `perfectionist::no_star_imports`,
    /// not by this rule: a top-level glob is left alone under `item`.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. None of
    /// the three shapes is wrong in the abstract — the violation is a
    /// mismatch with the project's configured `style`. Enforcing one
    /// keeps `use` blocks scanning uniformly and makes import diffs
    /// predictable. rustfmt can enforce the same shape, but only on the
    /// nightly channel; this lint gives stable-toolchain projects a hard
    /// CI check instead of a silent reformat.
    ///
    /// ### Example
    /// Under the default `style = "module"`:
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use std::collections::BTreeMap;
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// use std::collections::{BTreeMap, HashMap};
    /// ```
    pub perfectionist::IMPORT_GRANULARITY,
    Warn,
    "import granularity does not match the configured `import_granularity.style`",
    report_in_external_macro: false
}

/// Active by default. `module` is the shipped baseline; a project that
/// prefers `crate` or `item` sets `style` in `dylint.toml`. Read by
/// `register_pass`; gen-docs picks the constant up to render the rule's
/// default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

const CONFIG_KEY: &str = "perfectionist::import_granularity";

pub struct ImportGranularity {
    style: Style,
    respect_cfg_blocks: bool,
    respect_visibility: bool,
    respect_doc_comments: bool,
}

impl ImportGranularity {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            style: config.style,
            respect_cfg_blocks: config.respect_cfg_blocks,
            respect_visibility: config.respect_visibility,
            respect_doc_comments: config.respect_doc_comments,
        }
    }
}

impl_lint_pass!(ImportGranularity => [IMPORT_GRANULARITY]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[IMPORT_GRANULARITY]);
}

/// Install this rule's pass.
pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("import_granularity", DEFAULT_STATE) {
        return;
    }
    // Pre-expansion: `#[cfg(...)]` attributes are evaluated and stripped
    // during macro expansion, so a post-expansion pass can't see them
    // and `respect_cfg_blocks` would be a no-op. Running before
    // expansion keeps the cfg gates (and the source's original `use`
    // structure) intact.
    lint_store.register_pre_expansion_pass(|| Box::new(ImportGranularity::new()));
}

impl EarlyLintPass for ImportGranularity {
    fn check_crate(&mut self, lint_context: &EarlyContext<'_>, krate: &Crate) {
        self.check_items(lint_context, &krate.items);
    }

    fn check_item(&mut self, lint_context: &EarlyContext<'_>, item: &Item) {
        if let ItemKind::Mod(_, _, ModKind::Loaded(items, _, _)) = &item.kind {
            self.check_items(lint_context, items);
        }
    }
}

/// One `use` statement that has been admitted into the group analysis.
struct UseEntry<'ast> {
    item: &'ast Item,
    info: StmtInfo,
    /// Source text of every attribute, in order — reproduced verbatim
    /// onto each rendered statement.
    attrs: Vec<String>,
    /// Source text of the non-doc attributes only (`#[cfg(...)]`,
    /// `#[allow(...)]`, etc.). Two statements that differ here can't be
    /// merged without changing what compiles, so the rewrite is
    /// withheld; doc comments are excluded because dropping one only
    /// loses documentation.
    nondoc_attrs: Vec<String>,
    /// Trailing-space-terminated visibility text (`"pub "`), or empty.
    vis: String,
    /// What decides whether two adjacent statements may share a group.
    group_key: (String, Vec<String>),
    /// A doc-commented statement (under `respect_doc_comments`) is never
    /// merged with a neighbour.
    force_singleton: bool,
    /// Lowest byte position to replace — the start of the first
    /// attribute, or of the `use` keyword when there are none.
    lo: BytePos,
}

enum AttrClass {
    Doc,
    Cfg,
    Other,
}

fn attr_class(attr: &Attribute) -> AttrClass {
    if matches!(attr.kind, AttrKind::DocComment(..)) || attr.has_name(sym::doc) {
        AttrClass::Doc
    } else if attr.has_name(sym::cfg) || attr.has_name(sym::cfg_attr) {
        AttrClass::Cfg
    } else {
        AttrClass::Other
    }
}

impl ImportGranularity {
    fn check_items(&self, lint_context: &EarlyContext<'_>, items: &[Box<Item>]) {
        let mut group: Vec<UseEntry<'_>> = Vec::new();
        let mut group_key: Option<(String, Vec<String>)> = None;
        for item in items {
            match self.use_entry(lint_context, item) {
                // A non-`use` item, a macro-expanded `use`, or one the
                // rule declines to rewrite ends the current run.
                None => {
                    self.process_group(lint_context, &group);
                    group.clear();
                    group_key = None;
                }
                Some(entry) if entry.force_singleton => {
                    self.process_group(lint_context, &group);
                    group.clear();
                    group_key = None;
                    self.process_group(lint_context, std::slice::from_ref(&entry));
                }
                Some(entry) => {
                    if group_key.as_ref() != Some(&entry.group_key) {
                        self.process_group(lint_context, &group);
                        group.clear();
                        group_key = Some(entry.group_key.clone());
                    }
                    group.push(entry);
                }
            }
        }
        self.process_group(lint_context, &group);
    }

    fn use_entry<'ast>(
        &self,
        lint_context: &EarlyContext<'_>,
        item: &'ast Item,
    ) -> Option<UseEntry<'ast>> {
        let ItemKind::Use(tree) = &item.kind else {
            return None;
        };
        if item.span.from_expansion() {
            return None;
        }
        let info = stmt_info(tree)?;
        let source_map = lint_context.sess().source_map();

        let mut attrs = Vec::with_capacity(item.attrs.len());
        let mut nondoc_attrs = Vec::new();
        let mut attr_key = Vec::new();
        for attr in &item.attrs {
            let snippet = source_map.span_to_snippet(attr.span).ok()?;
            let class = attr_class(attr);
            let include = match class {
                AttrClass::Doc => self.respect_doc_comments,
                AttrClass::Cfg => self.respect_cfg_blocks,
                AttrClass::Other => true,
            };
            if include {
                attr_key.push(snippet.clone());
            }
            if !matches!(class, AttrClass::Doc) {
                nondoc_attrs.push(snippet.clone());
            }
            attrs.push(snippet);
        }
        attr_key.sort();

        let vis = vis_text(lint_context, &item.vis);
        let vis_key = if self.respect_visibility {
            vis.clone()
        } else {
            String::new()
        };
        let force_singleton = self.respect_doc_comments
            && item
                .attrs
                .iter()
                .any(|attr| matches!(attr_class(attr), AttrClass::Doc));

        let lo = item
            .attrs
            .iter()
            .map(|attr| attr.span.lo())
            .chain(std::iter::once(item.span.lo()))
            .min()
            .unwrap_or(item.span.lo());

        Some(UseEntry {
            item,
            info,
            attrs,
            nondoc_attrs,
            vis,
            group_key: (vis_key, attr_key),
            force_singleton,
            lo,
        })
    }

    fn process_group(&self, lint_context: &EarlyContext<'_>, group: &[UseEntry<'_>]) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            return;
        };
        let stmts: Vec<&StmtInfo> = group.iter().map(|entry| &entry.info).collect();
        if is_compliant(self.style, &stmts) {
            return;
        }

        let leaves: Vec<Leaf> = group
            .iter()
            .flat_map(|entry| entry.info.leaves.iter().cloned())
            .collect();
        let bodies = render::render(self.style, &leaves);
        if bodies.is_empty() {
            return;
        }

        let replace_span = first
            .item
            .span
            .with_lo(first.lo)
            .with_hi(last.item.span.hi());

        // A merge across statements that differ in visibility or in
        // non-doc attributes (only reachable when `respect_visibility` /
        // `respect_cfg_blocks` is off) cannot preserve what compiles or
        // what is exported. Flag the group but withhold a mechanical
        // fix, rather than silently rewriting semantics.
        if group
            .iter()
            .any(|entry| entry.vis != first.vis || entry.nondoc_attrs != first.nondoc_attrs)
        {
            span_lint_and_then(
                lint_context,
                IMPORT_GRANULARITY,
                replace_span,
                self.message(),
                |diagnostic| {
                    diagnostic.help(
                        "these statements differ in visibility or `#[cfg(...)]`; \
                         merge them by hand to avoid changing what is compiled or exported",
                    );
                },
            );
            return;
        }

        let indent = indent_of(lint_context, first.item.span).unwrap_or(0);
        let pad = " ".repeat(indent);
        let mut prefix = String::new();
        for attr in &first.attrs {
            prefix.push_str(attr);
            prefix.push('\n');
            prefix.push_str(&pad);
        }
        prefix.push_str(&first.vis);
        let replacement = bodies
            .iter()
            .map(|body| format!("{prefix}use {body};"))
            .collect::<Vec<_>>()
            .join(&format!("\n{pad}"));

        // Down to `MaybeIncorrect` when applying the fix would drop
        // something the rewrite can't carry: an inline comment inside the
        // replaced span, or a doc comment that differs across the merged
        // statements (kept only from the first).
        let has_comment = lint_context
            .sess()
            .source_map()
            .span_to_snippet(replace_span)
            .is_ok_and(|snippet| snippet.contains("//") || snippet.contains("/*"));
        let drops_doc = group.iter().any(|entry| entry.attrs != first.attrs);
        let applicability = if has_comment || drops_doc {
            Applicability::MaybeIncorrect
        } else {
            Applicability::MachineApplicable
        };

        span_lint_and_then(
            lint_context,
            IMPORT_GRANULARITY,
            replace_span,
            self.message(),
            |diagnostic| {
                diagnostic.span_suggestion(
                    replace_span,
                    "reorganize the imports",
                    replacement,
                    applicability,
                );
            },
        );
    }

    fn message(&self) -> &'static str {
        match self.style {
            Style::Crate => "imports are not collapsed to one `use` per crate root",
            Style::Module => "imports are not grouped into one `use` per module",
            Style::Item => "imports are not split into one `use` per item",
        }
    }
}

fn vis_text(lint_context: &EarlyContext<'_>, vis: &Visibility) -> String {
    if matches!(vis.kind, VisibilityKind::Inherited) {
        return String::new();
    }
    match lint_context.sess().source_map().span_to_snippet(vis.span) {
        Ok(snippet) => format!("{snippet} "),
        Err(_) => String::new(),
    }
}
