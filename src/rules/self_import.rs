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

use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Block, Crate, Item, ItemKind, ModKind, Stmt, StmtKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use crate::common::{DefaultState, resolved_state};

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
    lint_store.register_early_pass(move || {
        Box::new(SelfImport {
            style: config.style,
        })
    });
}

impl EarlyLintPass for SelfImport {
    fn check_crate(&mut self, cx: &EarlyContext<'_>, krate: &Crate) {
        let mut walker = SelfImportWalker {
            cx,
            style: self.style,
        };
        walker.scan_items(krate.items.iter().map(|item| Some(&**item)));
        visit::walk_crate(&mut walker, krate);
    }
}

/// Drives the rule across every module body and block in the crate.
/// `check_crate`'s own pass handles the crate's top-level items; the
/// `Visitor` impl then descends into nested modules and block bodies so
/// the adjacency window (`combined`) and the per-tree rewrite
/// (`forbid`) both see each `use` in its source-ordered sibling list.
struct SelfImportWalker<'a, 'tcx> {
    cx: &'a EarlyContext<'tcx>,
    style: Style,
}

impl SelfImportWalker<'_, '_> {
    /// Process one source-ordered sequence of entries: fold adjacent
    /// imports under `combined`, and rewrite each `self`-importing
    /// `use` under `forbid`. Each entry is `Some(item)` for an item in
    /// position, or `None` for an intervening statement (a `let`, an
    /// expression) that breaks the `combined` adjacency window.
    fn scan_items<'ast>(&self, entries: impl Iterator<Item = Option<&'ast Item>> + Clone) {
        if let Style::Combined = self.style {
            combined::scan(self.cx, entries.clone());
        }
        if let Style::Forbid = self.style {
            for item in entries.flatten() {
                if let ItemKind::Use(tree) = &item.kind
                    && !item.span.from_expansion()
                {
                    forbid::check_use_item(self.cx, item, tree);
                }
            }
        }
    }
}

impl<'ast> Visitor<'ast> for SelfImportWalker<'_, '_> {
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
