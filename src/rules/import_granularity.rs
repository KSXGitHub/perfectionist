use std::collections::HashSet;
use std::sync::Arc;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::source::indent_of;
use rustc_ast::{
    AttrKind, Attribute, Crate, Item, ItemKind, MetaItemKind, ModKind, Visibility, VisibilityKind,
};
use rustc_errors::emitter::SilentEmitter;
use rustc_errors::{Applicability, DiagCtxt};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_session::parse::ParseSess;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, FileName, SourceFile, Span, sym};

mod check;
mod config;
mod model;
mod render;

use check::is_compliant;
use config::{Config, Style};
use model::{Leaf, StmtInfo, stmt_info};

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::find_enclosing_hir_ids;

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
    // Late pass: out-of-line `mod foo;` modules are `ModKind::Unloaded`
    // until macro expansion, so a pre-expansion pass never sees them.
    // `check_crate` re-parses each source file instead, which reaches
    // every submodule while keeping `#[cfg(...)]` gates intact (parsing
    // does not strip cfg, unlike the post-expansion AST).
    lint_store.register_late_pass(|_| Box::new(ImportGranularity::new()));
}

/// A detected violation parked until the enclosing HIR node is known.
/// Emission happens through [`span_lint_hir_and_then`] so a per-module /
/// per-item `#[allow]` / `#[expect]` resolves (see
/// [`crate::enclosing_hir`]).
struct Pending {
    /// The span used to resolve the lint-level anchor: the first `use`
    /// statement of the group, always contained by its own HIR node.
    anchor: Span,
    /// The span the diagnostic points at and rewrites — the whole run of
    /// merged statements.
    span: Span,
    violation: Violation,
}

/// What to render for a [`Pending`] violation once its anchor is known.
enum Violation {
    /// The group is flagged but no mechanical fix is offered, because
    /// merging would change what is compiled or exported.
    ManualMerge,
    /// The group can be rewritten into `replacement`.
    Reorganize {
        replacement: String,
        applicability: Applicability,
    },
}

impl<'tcx> LateLintPass<'tcx> for ImportGranularity {
    fn check_crate(&mut self, lint_context: &LateContext<'tcx>) {
        let tcx = lint_context.tcx;
        let source_map = lint_context.sess().psess.clone_source_map();

        // The files that define a module in this crate's module tree.
        // Re-parsing is restricted to these so that source-map files
        // which are *not* standalone modules — `include_str!` data and
        // `include!` fragments — are not re-parsed and wrongly flagged.
        let mut module_files: HashSet<FileName> = HashSet::new();
        record_module_file(&source_map, &mut module_files, tcx.hir_root_module().spans);
        for item_id in tcx.hir_free_items() {
            if let rustc_hir::ItemKind::Mod(_, module) = &tcx.hir_item(item_id).kind {
                record_module_file(&source_map, &mut module_files, module.spans);
            }
        }

        // Snapshot the files before parsing: re-parsing takes a write
        // lock on the shared source map, so it must not run while the
        // `files()` read guard is held.
        let module_source_files: Vec<Arc<SourceFile>> = {
            let source_files = source_map.files();
            source_files
                .iter()
                .filter(|source_file| source_file.cnum == LOCAL_CRATE)
                .filter(|source_file| module_files.contains(&source_file.name))
                .cloned()
                .collect()
        };

        // A throwaway `ParseSess` sharing the real `SourceMap` (so spans,
        // and our suggestions, point at the real files) but with a
        // silenced `DiagCtxt`, so a file that does not parse cleanly
        // standalone is skipped rather than surfacing parse errors.
        let parse_psess = ParseSess::with_dcx(
            DiagCtxt::new(Box::new(SilentEmitter)),
            Arc::clone(&source_map),
        );

        let mut violations: Vec<Pending> = Vec::new();
        for source_file in &module_source_files {
            if let Some(krate) = parse_module_file(&parse_psess, source_file) {
                self.check_items(lint_context, &krate.items, &mut violations);
            }
        }

        if violations.is_empty() {
            return;
        }

        // Anchor each violation at its enclosing HIR node so a per-module
        // / per-item `#[allow]` resolves (emitting from `check_crate`
        // alone would sit at the crate root). Resolve on the first `use`'s
        // own span, not the merged replacement span: an out-of-line
        // `mod foo;` item's span lives in the parent file, so a merged
        // span there would fall back to the crate root.
        let anchors: Vec<Span> = violations.iter().map(|pending| pending.anchor).collect();
        let hir_ids = find_enclosing_hir_ids(tcx, &anchors);
        for (pending, hir_id) in violations.into_iter().zip(hir_ids) {
            let Pending {
                span, violation, ..
            } = pending;
            span_lint_hir_and_then(
                lint_context,
                IMPORT_GRANULARITY,
                hir_id,
                span,
                self.message(),
                |diagnostic| match &violation {
                    Violation::ManualMerge => {
                        diagnostic.help(
                            "these statements differ in visibility or `#[cfg(...)]`; \
                             merge them by hand to avoid changing what is compiled or exported",
                        );
                    }
                    Violation::Reorganize {
                        replacement,
                        applicability,
                    } => {
                        diagnostic.span_suggestion(
                            span,
                            "reorganize the imports",
                            replacement.clone(),
                            *applicability,
                        );
                    }
                },
            );
        }
    }
}

/// Record the on-disk source file that holds a module's body, keyed by
/// name. A dummy span (no real body) contributes nothing. Only
/// [`FileName::Real`] files count: a module synthesised by a proc macro
/// has a `<proc-macro source>` file that must not be re-parsed and
/// flagged as if the user wrote it.
fn record_module_file(
    source_map: &SourceMap,
    module_files: &mut HashSet<FileName>,
    spans: rustc_hir::ModSpans,
) {
    let inner_span = spans.inner_span;
    if inner_span.is_dummy() {
        return;
    }
    let name = &source_map.lookup_source_file(inner_span.lo()).name;
    if matches!(name, FileName::Real(_)) {
        module_files.insert(name.clone());
    }
}

/// Re-parse a module's source file from its already-loaded text. Returns
/// `None` (silently discarding buffered diagnostics — `parse_psess` is
/// wired to a [`SilentEmitter`]) when the file does not parse as a
/// standalone module. The shared source map already holds this file and
/// deduplicates by name, so the parser reuses the loaded `SourceFile`
/// (preserving the real spans) and the passed source text is ignored —
/// hence the empty string, which avoids both a disk re-read and a clone
/// of the whole file.
fn parse_module_file(parse_psess: &ParseSess, source_file: &SourceFile) -> Option<Crate> {
    // Load-bearing: a `SourceFile` without in-memory source makes the
    // lexer ICE ("cannot lex `source_file` without source"). Local-crate
    // `Real` files normally carry it, but bail rather than risk the ICE.
    source_file.src.as_ref()?;
    let mut parser = match new_parser_from_source_str(
        parse_psess,
        source_file.name.clone(),
        String::new(),
        StripTokens::ShebangAndFrontmatter,
    ) {
        Ok(parser) => parser,
        Err(errors) => {
            for error in errors {
                error.cancel();
            }
            return None;
        }
    };
    match parser.parse_crate_mod() {
        Ok(krate) => Some(krate),
        Err(error) => {
            error.cancel();
            None
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
    // Only documentation *text* counts as a doc comment: `///`, `//!`,
    // and `#[doc = "..."]`. The list form `#[doc(hidden)]` /
    // `#[doc(alias = ...)]` changes the item's API surface, so it is a
    // real attribute that must keep statements apart like any other.
    let is_doc_comment = matches!(attr.kind, AttrKind::DocComment(..))
        || (attr.has_name(sym::doc)
            && matches!(attr.meta_kind(), Some(MetaItemKind::NameValue(_))));
    if is_doc_comment {
        AttrClass::Doc
    } else if attr.has_name(sym::cfg) || attr.has_name(sym::cfg_attr) {
        AttrClass::Cfg
    } else {
        AttrClass::Other
    }
}

impl ImportGranularity {
    fn check_items(
        &self,
        lint_context: &LateContext<'_>,
        items: &[Box<Item>],
        violations: &mut Vec<Pending>,
    ) {
        let mut group: Vec<UseEntry<'_>> = Vec::new();
        let mut group_key: Option<(String, Vec<String>)> = None;
        for item in items {
            match self.use_entry(lint_context, item) {
                // A non-`use` item, a macro-expanded `use`, or one the
                // rule declines to rewrite ends the current run.
                None => {
                    self.process_group(lint_context, &group, violations);
                    group.clear();
                    group_key = None;
                }
                Some(entry) if entry.force_singleton => {
                    self.process_group(lint_context, &group, violations);
                    group.clear();
                    group_key = None;
                    self.process_group(lint_context, std::slice::from_ref(&entry), violations);
                }
                Some(entry) => {
                    if group_key.as_ref() != Some(&entry.group_key) {
                        self.process_group(lint_context, &group, violations);
                        group.clear();
                        group_key = Some(entry.group_key.clone());
                    }
                    group.push(entry);
                }
            }
        }
        self.process_group(lint_context, &group, violations);

        // Descend into inline `mod { ... }` bodies. Out-of-line
        // `mod foo;` modules are `ModKind::Unloaded` here (a fresh parse
        // does not load them), but their files appear in the source map
        // in their own right and are re-parsed by `check_crate`.
        for item in items {
            if let ItemKind::Mod(_, _, ModKind::Loaded(items, _, _)) = &item.kind {
                self.check_items(lint_context, items, violations);
            }
        }
    }

    fn use_entry<'ast>(
        &self,
        lint_context: &LateContext<'_>,
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
        // Compared as an order-insensitive set: two statements carrying
        // the same attributes in a different written order can still
        // merge safely.
        nondoc_attrs.sort();

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

    fn process_group(
        &self,
        lint_context: &LateContext<'_>,
        group: &[UseEntry<'_>],
        violations: &mut Vec<Pending>,
    ) {
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
            violations.push(Pending {
                anchor: first.item.span,
                span: replace_span,
                violation: Violation::ManualMerge,
            });
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

        violations.push(Pending {
            anchor: first.item.span,
            span: replace_span,
            violation: Violation::Reorganize {
                replacement,
                applicability,
            },
        });
    }

    fn message(&self) -> &'static str {
        match self.style {
            Style::Crate => "imports are not collapsed to one `use` per crate root",
            Style::Module => "imports are not grouped into one `use` per module",
            Style::Item => "imports are not split into one `use` per item",
        }
    }
}

fn vis_text(lint_context: &LateContext<'_>, vis: &Visibility) -> String {
    if matches!(vis.kind, VisibilityKind::Inherited) {
        return String::new();
    }
    match lint_context.sess().source_map().span_to_snippet(vis.span) {
        Ok(snippet) => format!("{snippet} "),
        Err(_) => String::new(),
    }
}
