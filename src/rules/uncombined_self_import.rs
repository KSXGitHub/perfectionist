//! `perfectionist::uncombined_self_import` — fold a module import and an
//! adjacent item import from that module into a single
//! `use module::{self, item};`.
//!
//! The rule is inactive by default; a project opts in via
//! `[perfectionist].enable`.
//!
//! Module layout:
//!
//! - [`render`] — `use`-tree rendering helpers.
//! - [`combined`] — the adjacency fold.
//!
//! The rule runs as a [`LateLintPass`] that **re-parses each of the
//! crate's module source files** via [`crate::module_reparse`]. A
//! pre-expansion pass would leave out-of-line `mod foo;` modules
//! `ModKind::Unloaded` (their files are not read until macro expansion),
//! so it would silently skip every separate-file submodule. Re-parsing
//! reaches every module-scoped submodule while keeping `#[cfg(...)]`
//! gates intact (parsing does not strip cfg, unlike the post-expansion
//! AST). The sibling `import_granularity_mismatch` rule shares the same machinery.

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::find_enclosing_hir_ids;
use crate::module_reparse::for_each_module_file;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Block, Item, ItemKind, ModKind, Stmt, StmtKind};
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod combined;
mod render;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Folds two adjacent `use` statements — one importing a module
    /// and one importing an item from that same module — into a
    /// single `use module::{self, item};`. The rule is inactive by
    /// default; a project opts in via `[perfectionist].enable`.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// project that wants a module and the items it re-exports to
    /// travel under one `use` enables this rule so the grouping is
    /// applied uniformly rather than decided case by case:
    ///
    /// ```toml
    /// [perfectionist]
    /// enable = ["uncombined_self_import"]
    /// ```
    ///
    /// The autofix is `MaybeIncorrect` whenever it narrows the
    /// namespaces an import brings into scope: folding a bare
    /// `use module;` into `{self, ...}` imports only the module,
    /// while the bare form imports every namespace named `module`
    /// (type, value, macro) — a difference that matters only in the
    /// rare case where a value or macro shares the module's name in
    /// the same parent.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::unnecessary_self_imports` (`restriction`, off by
    /// default) is the opposite policy: it flags a sole
    /// `use module::{self};` and unwraps it to `use module;`. It
    /// matches only the single-item `{self}` form, so it never
    /// touches the multi-item `{self, item}` groups this rule
    /// produces. The two are mirror images — pick one direction and
    /// do not enable both.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// use foo::bar;
    /// use foo::bar::Baz;
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// use foo::bar::{self, Baz};
    /// ```
    pub perfectionist::UNCOMBINED_SELF_IMPORT,
    Warn,
    "a module import and an adjacent item import from it can be combined through `self`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::uncombined_self_import";

pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Inactive;

/// Configuration is reserved for future knobs; the lint currently has
/// no options. The empty struct still exists so that a stray
/// `[perfectionist::uncombined_self_import]` table in `dylint.toml`
/// deserialises rather than producing a confusing parse error.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct UncombinedSelfImport;

impl_lint_pass!(UncombinedSelfImport => [UNCOMBINED_SELF_IMPORT]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNCOMBINED_SELF_IMPORT]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("uncombined_self_import", DEFAULT_STATE) {
        return;
    }
    let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
    // Late pass: out-of-line `mod foo;` modules are `ModKind::Unloaded`
    // until macro expansion, so a pre-expansion pass never sees them.
    // `check_crate` re-parses each source file instead (see the module
    // docs), reaching every module-scoped submodule while keeping
    // `#[cfg(...)]` gates intact.
    lint_store.register_late_pass(|_| Box::new(UncombinedSelfImport));
}

/// A detected violation parked until its enclosing HIR node is known.
/// The rule discovers violations by re-parsing source files (see
/// [`UncombinedSelfImport::check_crate`]), outside the HIR walk, so
/// emission is deferred and routed through [`span_lint_hir_and_then`] at
/// the enclosing node — that is what lets a per-module / per-item
/// `#[allow]` / `#[expect]` resolve, instead of only a crate-root one.
pub(super) struct Pending {
    /// Resolves the lint-level anchor: the violating `use` item's own
    /// span, always contained by its HIR node.
    pub(super) anchor: Span,
    /// The span the diagnostic points at.
    pub(super) span: Span,
    pub(super) message: &'static str,
    pub(super) fix: Fix,
}

/// The fold edit for a [`Pending`]: rewrite the kept statement and
/// delete the folded one.
pub(super) struct Fix {
    pub(super) label: &'static str,
    pub(super) parts: Vec<(Span, String)>,
    pub(super) applicability: Applicability,
}

impl<'tcx> LateLintPass<'tcx> for UncombinedSelfImport {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        // Re-parse every module source file (reaching out-of-line
        // submodules while keeping `#[cfg(...)]` gates intact) and walk
        // each file's scopes in turn. See [`crate::module_reparse`].
        let mut violations: Vec<Pending> = Vec::new();
        for_each_module_file(cx, |krate| {
            let mut walker = UncombinedSelfImportWalker {
                cx,
                violations: &mut violations,
            };
            walker.scan_items(krate.items.iter().map(|item| Some(&**item)));
            visit::walk_crate(&mut walker, krate);
        });

        if violations.is_empty() {
            return;
        }

        // Anchor each violation at its enclosing HIR node so a per-module
        // / per-item `#[allow]` resolves (emitting from `check_crate`
        // alone would sit at the crate root).
        let anchors: Vec<Span> = violations.iter().map(|pending| pending.anchor).collect();
        let hir_ids = find_enclosing_hir_ids(cx.tcx, &anchors);
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
        UNCOMBINED_SELF_IMPORT,
        hir_id,
        span,
        message,
        |diagnostic| {
            diagnostic.multipart_suggestion(fix.label, fix.parts, fix.applicability);
        },
    );
}

/// Walks one re-parsed module file, scanning each scope that holds a
/// source-ordered run of items — the file root, every inline `mod { ... }`
/// body, and every block. Out-of-line `mod foo;` modules are
/// `ModKind::Unloaded` in a fresh parse, but their files are re-parsed in
/// their own right by [`UncombinedSelfImport::check_crate`], so this walk
/// stays within a single file.
struct UncombinedSelfImportWalker<'a, 'b, 'tcx> {
    cx: &'a LateContext<'tcx>,
    violations: &'b mut Vec<Pending>,
}

impl UncombinedSelfImportWalker<'_, '_, '_> {
    /// Process one source-ordered sequence of entries, folding each
    /// adjacent module + item import pair. Each entry is `Some(item)`
    /// for an item in position, or `None` for an intervening statement
    /// (a `let`, an expression) that breaks the adjacency window.
    fn scan_items<'ast>(&mut self, entries: impl Iterator<Item = Option<&'ast Item>>) {
        combined::scan(self.cx, entries, self.violations);
    }
}

impl<'ast> Visitor<'ast> for UncombinedSelfImportWalker<'_, '_, '_> {
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
