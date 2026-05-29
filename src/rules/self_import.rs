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
//! crate's module source files** via [`crate::module_reparse`]. A
//! pre-expansion pass would leave out-of-line `mod foo;` modules
//! `ModKind::Unloaded` (their files are not read until macro expansion),
//! so it would silently skip every separate-file submodule. Re-parsing
//! reaches every module-scoped submodule while keeping `#[cfg(...)]`
//! gates intact (parsing does not strip cfg, unlike the post-expansion
//! AST). The sibling `import_granularity` rule shares the same machinery.

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Block, Item, ItemKind, ModKind, Stmt, StmtKind};
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::find_enclosing_hir_ids;
use crate::module_reparse::for_each_module_file;

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
    // docs), reaching every module-scoped submodule while keeping
    // `#[cfg(...)]` gates intact.
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
        // Re-parse every module source file (reaching out-of-line
        // submodules while keeping `#[cfg(...)]` gates intact) and walk
        // each file's scopes in turn. See [`crate::module_reparse`].
        let style = self.style;
        let mut violations: Vec<Pending> = Vec::new();
        for_each_module_file(cx, |krate| {
            let mut walker = SelfImportWalker {
                cx,
                style,
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
