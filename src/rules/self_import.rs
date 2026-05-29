//! `perfectionist::self_import` — enforce a project-wide policy for
//! how `self` appears in `use` statements.
//!
//! The rule is inactive by default and direction-less: a project that
//! adopts it picks `forbid` (always prefer the bare `use foo::bar;`) or
//! `combined` (fold adjacent module + item imports into
//! `use foo::bar::{self, X};`). `style` is therefore mandatory whenever
//! the rule is enabled.
//!
//! Module layout:
//!
//! - [`render`] — `use`-tree rendering helpers shared by both styles.
//! - [`forbid`] — the `forbid` style's per-tree rewrite.
//! - [`combined`] — the `combined` style's adjacency fold.
//!
//! The rule runs as a [`LateLintPass`] that **re-parses each of the
//! crate's module source files** from a throwaway [`ParseSess`] sharing
//! the real [`SourceMap`]. A pre-expansion pass would leave out-of-line
//! `mod foo;` modules `ModKind::Unloaded` (their files are not read
//! until macro expansion), so it would silently skip every separate-file
//! submodule. Re-parsing reaches every submodule while keeping
//! `#[cfg(...)]` gates intact (parsing does not strip cfg, unlike the
//! post-expansion AST). This mirrors the sibling `import_granularity`
//! rule.

use std::collections::HashSet;
use std::sync::Arc;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Block, Crate, Item, ItemKind, ModKind, Stmt, StmtKind};
use rustc_errors::emitter::SilentEmitter;
use rustc_errors::{Applicability, DiagCtxt};
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_session::parse::ParseSess;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::source_map::SourceMap;
use rustc_span::{FileName, SourceFile, Span};

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::find_enclosing_hir_ids;

mod combined;
mod forbid;
mod render;

declare_tool_lint! {
    /// ### What it does
    /// Enforces a project-wide policy for naming a module's own export
    /// through `self` in `use` statements. The rule is inactive by
    /// default; a project opts in and sets `style` to one of:
    ///
    /// - `forbid` — every form that imports a module via `self` is a
    ///   violation. `use foo::bar::{self};`, the brace-nested
    ///   `use foo::{bar::self};` form, and the `self` member of
    ///   `use foo::bar::{self, Baz};` are all rewritten to the bare
    ///   `use foo::bar;` (the braced-with-items form splits the module
    ///   import out into its own statement). The bare `use foo::bar::self;`
    ///   (no braces) is a hard error in current Rust, so the rule only
    ///   encounters the brace-list forms.
    /// - `combined` — two adjacent statements that import a module and
    ///   an item from it (`use foo::bar; use foo::bar::Baz;`) fold into
    ///   a single `use foo::bar::{self, Baz};`.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. Both
    /// directions are coherent; a project simply picks one and applies
    /// it everywhere so `self`-in-`use` decisions stop being made
    /// case by case. The rule is inactive by default; enable it per
    /// crate and pick a direction in `dylint.toml`:
    ///
    /// ```toml
    /// [perfectionist]
    /// enable = ["self_import"]
    /// ```
    ///
    /// The autofix is always `MaybeIncorrect` when it changes the
    /// namespaces an import brings into scope. `use foo::bar;` imports
    /// every namespace named `bar` (type, value, macro), while
    /// `use foo::bar::{self};` imports only the module — a difference
    /// that matters only in the rare case where a value or macro shares
    /// the module's name in the same parent.
    ///
    /// ### Example
    /// ```rust,ignore
    /// // style = "forbid"
    /// use foo::bar::{self};
    /// use foo::qux::{self, Baz};
    /// ```
    /// Use instead (each statement is fixed independently):
    /// ```rust,ignore
    /// use foo::bar;
    /// use foo::qux;
    /// use foo::qux::Baz;
    /// ```
    ///
    /// ```rust,ignore
    /// // style = "combined"
    /// use foo::bar;
    /// use foo::bar::Baz;
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// use foo::bar::{self, Baz};
    /// ```
    pub perfectionist::SELF_IMPORT,
    Warn,
    "module imported through `self` against the project's configured `self`-import style",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::self_import";

pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Inactive;

/// The direction this rule enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Style {
    /// Forbid every `self`-as-module form; prefer the bare module
    /// import.
    Forbid,
    /// Fold adjacent module + item imports into a single
    /// `module::{self, item}`.
    Combined,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    // A bare `Style` (not `Option<Style>`) with no `serde(default)`, so
    // `style` is a required field: an enabled rule with no `style` fails
    // to deserialize rather than silently defaulting to a direction.
    // This is also the syntactic signal gen-docs reads to badge the
    // field `mandatory`. The config is read only when the rule is
    // enabled (see `register_pass`), so a disabled rule never needs it.
    /// The `self`-import direction to enforce: `forbid` or `combined`.
    /// It has no default — the two directions are opposites with no
    /// neutral baseline — so it must be set when the rule is enabled.
    style: Style,
}

pub struct SelfImport {
    style: Style,
}

impl_lint_pass!(SelfImport => [SELF_IMPORT]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SELF_IMPORT]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("self_import", DEFAULT_STATE) {
        return;
    }
    // The rule is enabled, so `style` is mandatory and has no default.
    // Read it with `config` rather than `config_or_default`: the latter
    // needs `Config: Default`, which would force a default direction.
    // `config` instead returns `Ok(None)` when the table is absent and
    // `Err` when it is present but `style` is missing or invalid — both
    // are configuration errors we fail loudly on.
    let config = dylint_linting::config::<Config>(CONFIG_KEY)
        .unwrap_or_else(|error| {
            panic!(
                "perfectionist::self_import: invalid `[perfectionist::self_import]` \
                 configuration: {error}",
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "perfectionist::self_import is enabled but `style` is not set; add \
                 `style = \"forbid\"` or `style = \"combined\"` under \
                 `[perfectionist::self_import]` in dylint.toml",
            )
        });
    // Late pass: out-of-line `mod foo;` modules are `ModKind::Unloaded`
    // until macro expansion, so a pre-expansion pass never sees them.
    // `check_crate` re-parses each source file instead (see the module
    // docs), reaching every submodule while keeping `#[cfg(...)]` gates
    // intact.
    lint_store.register_late_pass(move |_| {
        Box::new(SelfImport {
            style: config.style,
        })
    });
}

/// A detected violation parked until its enclosing HIR node is known.
/// The rule discovers violations by re-parsing source files (see
/// [`SelfImport::check_crate`]), outside the HIR walk, so emission is
/// deferred and routed through [`span_lint_hir_and_then`] at the
/// enclosing node — that is what lets a per-module / per-item `#[allow]`
/// / `#[expect]` resolve, instead of only a crate-root one.
pub(super) struct Pending {
    /// Resolves the lint-level anchor: the violating `use` item's own
    /// span, always contained by its HIR node.
    pub(super) anchor: Span,
    /// The span the diagnostic points at.
    pub(super) span: Span,
    pub(super) message: &'static str,
    pub(super) fix: Fix,
}

/// What to render for a [`Pending`] once its anchor is known.
pub(super) enum Fix {
    /// A single-span replacement (the `forbid` rewrites). Always
    /// `MaybeIncorrect`: the bare form imports every namespace named by
    /// the final segment, while the `self` form imports only the module.
    Replace {
        label: &'static str,
        replacement: String,
        note: Option<&'static str>,
    },
    /// A multi-part edit (`combined`'s fold: rewrite the kept statement,
    /// delete the folded one).
    Multipart {
        label: &'static str,
        parts: Vec<(Span, String)>,
        applicability: Applicability,
    },
}

impl<'tcx> LateLintPass<'tcx> for SelfImport {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        let tcx = cx.tcx;
        let source_map = cx.sess().psess.clone_source_map();

        // The files that define a module in this crate's module tree.
        // Re-parsing is restricted to these so source-map files that are
        // *not* standalone modules — `include_str!` data, `include!`
        // fragments — are not re-parsed and wrongly flagged.
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
                let mut walker = SelfImportWalker {
                    cx,
                    style: self.style,
                    violations: &mut violations,
                };
                walker.scan_items(krate.items.iter().map(|item| Some(&**item)));
                visit::walk_crate(&mut walker, &krate);
            }
        }

        if violations.is_empty() {
            return;
        }

        // Anchor each violation at its enclosing HIR node so a per-module
        // / per-item `#[allow]` resolves (emitting from `check_crate`
        // alone would sit at the crate root).
        let anchors: Vec<Span> = violations.iter().map(|pending| pending.anchor).collect();
        let hir_ids = find_enclosing_hir_ids(tcx, &anchors);
        for (pending, hir_id) in violations.into_iter().zip(hir_ids) {
            emit_pending(cx, hir_id, pending);
        }
    }
}

/// Emit one parked [`Pending`] at its resolved enclosing HIR node.
fn emit_pending(cx: &LateContext<'_>, hir_id: HirId, pending: Pending) {
    let Pending {
        span, message, fix, ..
    } = pending;
    span_lint_hir_and_then(
        cx,
        SELF_IMPORT,
        hir_id,
        span,
        message,
        |diagnostic| match fix {
            Fix::Replace {
                label,
                replacement,
                note,
            } => {
                if let Some(note) = note {
                    diagnostic.note(note);
                }
                diagnostic.span_suggestion(span, label, replacement, Applicability::MaybeIncorrect);
            }
            Fix::Multipart {
                label,
                parts,
                applicability,
            } => {
                diagnostic.multipart_suggestion(label, parts, applicability);
            }
        },
    );
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

/// Walks one re-parsed module file, scanning each scope that holds a
/// source-ordered run of items — the file root, every inline `mod { ... }`
/// body, and every block. Out-of-line `mod foo;` modules are
/// `ModKind::Unloaded` in a fresh parse, but their files are re-parsed in
/// their own right by [`SelfImport::check_crate`], so this walk stays
/// within a single file.
struct SelfImportWalker<'a, 'b, 'tcx> {
    cx: &'a LateContext<'tcx>,
    style: Style,
    violations: &'b mut Vec<Pending>,
}

impl SelfImportWalker<'_, '_, '_> {
    /// Process one source-ordered sequence of entries: fold adjacent
    /// imports under `combined`, and rewrite each `self`-importing `use`
    /// under `forbid`. Each entry is `Some(item)` for an item in
    /// position, or `None` for an intervening statement (a `let`, an
    /// expression) that breaks the `combined` adjacency window.
    fn scan_items<'ast>(&mut self, entries: impl Iterator<Item = Option<&'ast Item>> + Clone) {
        if let Style::Combined = self.style {
            combined::scan(self.cx, entries.clone(), self.violations);
        }
        if let Style::Forbid = self.style {
            for item in entries.flatten() {
                if let ItemKind::Use(tree) = &item.kind
                    && !item.span.from_expansion()
                {
                    forbid::check_use_item(self.cx, item, tree, self.violations);
                }
            }
        }
    }
}

impl<'ast> Visitor<'ast> for SelfImportWalker<'_, '_, '_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if let ItemKind::Mod(_, _, ModKind::Loaded(items, ..)) = &item.kind {
            self.scan_items(items.iter().map(|item| Some(&**item)));
        }
        visit::walk_item(self, item);
    }

    fn visit_block(&mut self, block: &'ast Block) {
        self.scan_items(block.stmts.iter().map(stmt_item));
        visit::walk_block(self, block);
    }
}

/// The item declared by an item statement, or `None` for any other
/// statement kind (which breaks the adjacency window).
fn stmt_item(stmt: &Stmt) -> Option<&Item> {
    match &stmt.kind {
        StmtKind::Item(item) => Some(item),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_values_deserialize() {
        assert_eq!(
            toml::from_str::<Config>(r#"style = "forbid""#)
                .unwrap()
                .style,
            Style::Forbid,
        );
        assert_eq!(
            toml::from_str::<Config>(r#"style = "combined""#)
                .unwrap()
                .style,
            Style::Combined,
        );
    }

    #[test]
    fn missing_style_is_an_error() {
        // `style` is a required field (bare `Style`, no `serde(default)`),
        // so an empty config table fails to deserialize rather than
        // silently defaulting to a direction. `register_pass` turns this
        // into the "enabled but no `style`" diagnostic. (The config is
        // only read for an enabled rule, so a disabled rule never hits
        // this.)
        assert!(toml::from_str::<Config>("").is_err());
    }

    #[test]
    fn unknown_style_is_rejected() {
        // There is no neutral `preserve` value; an unrecognised style is
        // a hard deserialisation error rather than a silent no-op.
        assert!(toml::from_str::<Config>(r#"style = "preserve""#).is_err());
    }
}
