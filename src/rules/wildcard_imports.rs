//! `perfectionist::wildcard_imports` — flag glob (`*`) `use` statements in
//! module bodies, with two configurable exceptions.
//!
//! The rule runs as a [`LateLintPass`] that **re-parses each of the
//! crate's module source files** via [`crate::module_reparse`]. The case
//! the rule cares about most — `use super::*;` inside a
//! `#[cfg(test)] mod tests` block — is cfg-gated, so the layout has to be
//! read from a re-parse that keeps `#[cfg(...)]` gates intact rather than
//! from the post-expansion AST (which strips them) or a pre-expansion
//! pass (which leaves out-of-line `mod foo;` modules `ModKind::Unloaded`
//! and so skips every separate-file submodule). The sibling
//! `import_grouping_mismatch` rule shares the same machinery, including the
//! `live_module_spans` guard that keeps the walk from descending into a
//! cfg-disabled inline module that is not part of the compiled crate.

use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::{Item, ItemKind, ModKind, UseTree, UseTreeKind, VisibilityKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, kw};
use std::collections::HashSet;

mod config;

use crate::common::DefaultState;
use crate::enclosing_hir::find_enclosing_hir_ids;
use crate::module_reparse::{SpanRange, parse_crate_module_files};
use config::{Config, Resolved};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags glob (`*`) `use` statements — `use foo::bar::*;` — in module
    /// bodies. These exceptions are enabled by default, and each can
    /// be turned off individually:
    ///
    /// - `prelude` — a glob whose final non-glob path segment names a
    ///   prelude module is allowed: `use rayon::prelude::*;`,
    ///   `use diesel::prelude::*;`. The recognised names are configurable
    ///   via `prelude_segment_names`.
    /// - `root_reexport` — a re-export glob (`pub use ...::*`) at the top
    ///   level of a module body is allowed: `pub use submodule::*;` in
    ///   `lib.rs`.
    ///
    /// The case the rule is most concerned with is `use super::*;` inside
    /// a `#[cfg(test)] mod tests` block; explicit imports must replace it.
    /// A project that wants a stricter posture disables either or both
    /// exceptions, or names extra always-allowed paths, in `dylint.toml`.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A glob
    /// import compiles and runs correctly; the project simply prefers
    /// explicit imports. Naming each imported item keeps a module's
    /// dependencies visible at the top of the file, stops an upstream
    /// addition from silently shadowing a local name, and makes a
    /// grep for where a name comes from land on a real `use`. The glob
    /// form is idiomatic for preludes and for root re-exports, so
    /// those are exempt by default.
    ///
    /// ### Why not `clippy::wildcard_imports`?
    ///
    /// Clippy's `wildcard_imports` (an allow-by-default `pedantic` lint)
    /// flags the same glob `use`s and, by default, exempts prelude
    /// imports, `pub use` re-exports, and `use super::*;` inside any
    /// module whose name contains `test` — overlapping this rule's
    /// exceptions. But all of those exemptions, together with the
    /// `allowed-wildcard-imports` path list, are coupled behind a single
    /// `warn-on-all-wildcard-imports` boolean: left at its default
    /// (`false`) the test-module `super::*;` carve-out is on, so
    /// `use super::*;` inside `mod tests` is *not* flagged; set to `true`,
    /// every exemption drops at once, including the prelude one. There is
    /// no setting that keeps `prelude::*` exempt while flagging
    /// `use super::*;` in a `#[cfg(test)] mod tests` block — which is
    /// exactly this rule's headline case. This rule decouples them: the
    /// `prelude` and `root_reexport` exceptions toggle independently
    /// (with a configurable `prelude_segment_names` and the `allowed_paths`
    /// escape hatch), and there is no test-module carve-out, so the test
    /// `super::*;` glob is flagged by default. Reach for
    /// `clippy::wildcard_imports` if its coarser, all-or-nothing exemption
    /// model is enough; reach for this rule when the test-module
    /// distinction matters. Paired with
    /// `perfectionist::named_prelude_imports` it expresses the project's
    /// full posture: preludes must be glob-imported, and globs are
    /// allowed only for preludes.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// #[cfg(test)]
    /// mod tests {
    ///     use super::*;
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// #[cfg(test)]
    /// mod tests {
    ///     use super::{ParsedThing, parse_thing};
    /// }
    /// ```
    ///
    /// **Not flagged:** the prelude and root-re-export exceptions.
    ///
    /// ```rust,ignore
    /// use rayon::prelude::*;
    /// pub use submodule::*;
    /// ```
    pub perfectionist::WILDCARD_IMPORTS,
    Warn,
    "glob (`*`) import in a module body, outside the prelude and root-re-export exceptions",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::wildcard_imports";

pub struct WildcardImports {
    config: Resolved,
}

impl_lint_pass!(WildcardImports => [WILDCARD_IMPORTS]);

impl Register for rule::WildcardImports {
    /// Active by default: both exceptions ship enabled, so the only
    /// globs flagged out of the box are non-prelude, non-re-export ones
    /// such as `use super::*;`.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[WILDCARD_IMPORTS]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Reject a misconfigured `allowed_paths` entry loudly: each must be an
        // absolute path (`crate::...` or `::<extern crate>::...`), otherwise it
        // could never match the absolute key the rule builds from a glob `use`.
        for entry in &config.allowed_paths {
            crate::abs_path::validate_absolute(entry).unwrap_or_else(|message| {
                panic!("perfectionist::wildcard_imports: {message}");
            });
        }
        // Late pass: the cfg-gated `#[cfg(test)] mod tests { use super::*; }`
        // case (and any out-of-line `mod foo;` submodule) is only reachable
        // by re-parsing each module file in a late pass — see the module
        // docs and [`crate::module_reparse`].
        lint_store.register_late_pass(move |_| {
            Box::new(WildcardImports {
                config: Resolved::from_config(config.clone()),
            })
        });
    }
}

/// A detected violation parked until the enclosing HIR node is known.
/// Emission happens through [`span_lint_hir_and_then`] so a per-module /
/// per-item `#[allow]` / `#[expect]` resolves (see
/// [`crate::enclosing_hir`]).
struct Pending {
    /// Span used to resolve the lint-level anchor: the violating `use`
    /// item's own span, always contained by its HIR node. Resolving on
    /// the glob's narrower span instead would still work, but the item
    /// span keeps the anchor robust for an out-of-line `mod foo;` whose
    /// item span lives in the parent file.
    anchor: Span,
    /// Span the diagnostic points at — the offending `path::*` subtree.
    span: Span,
}

impl<'tcx> LateLintPass<'tcx> for WildcardImports {
    fn check_crate(&mut self, lint_context: &LateContext<'tcx>) {
        let (crates, live_module_spans) = parse_crate_module_files(lint_context);
        let mut violations: Vec<Pending> = Vec::new();
        for krate in &crates {
            self.check_items(&krate.items, &live_module_spans, &mut violations);
        }
        if violations.is_empty() {
            return;
        }

        // Anchor each violation at its enclosing HIR node so a per-module
        // / per-item `#[allow]` resolves (emitting from `check_crate`
        // alone would sit at the crate root).
        let anchors: Vec<Span> = violations.iter().map(|pending| pending.anchor).collect();
        let hir_ids = find_enclosing_hir_ids(lint_context.tcx, &anchors);
        for (pending, hir_id) in violations.into_iter().zip(hir_ids) {
            span_lint_hir_and_then(
                lint_context,
                WILDCARD_IMPORTS,
                hir_id,
                pending.span,
                "glob import brings an unbounded set of names into scope",
                |diagnostic| {
                    diagnostic.help("import the specific items by name instead");
                },
            );
        }
    }
}

impl WildcardImports {
    /// Scan one module scope's items for glob `use` statements, then
    /// descend into each inline `mod { ... }` that is live in the
    /// compiled crate. Only module-scoped items are visited (the file
    /// root and inline `mod` bodies, never a block or function body), so
    /// every `use` examined here is already at the top level of a module
    /// body — which is what the `root_reexport` exception requires, on
    /// top of an explicit visibility.
    fn check_items(
        &self,
        items: &[Box<Item>],
        live_module_spans: &HashSet<SpanRange>,
        violations: &mut Vec<Pending>,
    ) {
        for item in items {
            if let ItemKind::Use(tree) = &item.kind
                && !item.span.from_expansion()
            {
                // The `root_reexport` exception is for a bare-`pub`
                // re-export glob (`pub use ...::*`), per the rule's
                // documented contract. A restricted visibility is not it:
                // `pub(crate)`/`pub(super)` are narrower re-exports the
                // rule still flags (use `allowed_paths` to permit them),
                // and `pub(self)` is private-equivalent — it re-exports
                // nothing, so exempting it would be plainly wrong.
                let is_reexport = matches!(item.vis.kind, VisibilityKind::Public);
                let mut globs: Vec<(Vec<String>, Span)> = Vec::new();
                collect_globs(tree, Vec::new(), &mut globs);
                for (module, glob_span) in globs {
                    if self.is_exempt(&module, is_reexport) {
                        continue;
                    }
                    violations.push(Pending {
                        anchor: item.span,
                        span: glob_span,
                    });
                }
            }
        }

        // Descend into inline `mod { ... }` bodies, but only those that
        // survived `#[cfg(...)]`-stripping into the compiled crate. The
        // re-parse keeps cfg-disabled modules (parsing does not strip
        // cfg), so without this guard a `#[cfg(test)] mod tests { ... }`
        // excluded from a non-test build would be linted — and, having no
        // HIR node, could not be suppressed by a local `#[allow]`.
        // Out-of-line `mod foo;` modules are `ModKind::Unloaded` here;
        // their files are re-parsed in their own right by `check_crate`.
        for item in items {
            if let ItemKind::Mod(_, _, ModKind::Loaded(items, _, mod_spans)) = &item.kind
                && live_module_spans
                    .contains(&(mod_spans.inner_span.lo(), mod_spans.inner_span.hi()))
            {
                self.check_items(items, live_module_spans, violations);
            }
        }
    }

    /// Whether the glob importing module path `module` is let through by
    /// an enabled exception. `is_reexport` is true when the enclosing
    /// `use` item carries bare `pub` visibility (the `root_reexport`
    /// exception's contract).
    fn is_exempt(&self, module: &[String], is_reexport: bool) -> bool {
        if self.config.prelude_exception
            && module
                .last()
                .is_some_and(|last| self.config.prelude_segment_names.contains(last))
        {
            return true;
        }
        // The `root_reexport` exception only ever applies to module-scoped
        // items: a `use` inside a block can't carry a visibility, so
        // `is_reexport` is already false there.
        if self.config.root_reexport_exception && is_reexport {
            return true;
        }
        // `allowed_paths` entries are absolute. `collect_globs` already
        // dropped any `PathRoot`, so a plain `use foo::bar::*` and a
        // `::`-rooted `use ::foo::bar::*` both arrive here as
        // `["foo", "bar"]`; `canonical_key` forms the absolute key that
        // matches either written form (`::foo::bar` for an extern crate,
        // `crate::internals` for a crate-root path). Only build the key when
        // there is an allow list to check it against — `allowed_paths` is
        // empty in the default config, so this skips the per-glob `String`
        // allocation in the common case.
        !self.config.allowed_paths.is_empty()
            && self
                .config
                .allowed_paths
                .contains(&crate::abs_path::canonical_key(&module.join("::")))
    }
}

/// Walk a `use` tree, collecting one entry per glob leaf: the segments of
/// the module the glob pulls from (`use rayon::prelude::*` →
/// `["rayon", "prelude"]`) and the span of that `path::*` subtree. A
/// `::`-rooted path segment (`use ::foo::*`) contributes no name, so the
/// global root never matches a prelude or allowed path.
fn collect_globs(tree: &UseTree, prefix: Vec<String>, out: &mut Vec<(Vec<String>, Span)>) {
    let mut path = prefix;
    for segment in &tree.prefix.segments {
        if segment.ident.name == kw::PathRoot {
            continue;
        }
        path.push(segment.ident.name.to_string());
    }
    match &tree.kind {
        UseTreeKind::Glob(_) => out.push((path, tree.span())),
        UseTreeKind::Nested { items, .. } => {
            // Each subtree extends its own copy of the accumulated prefix.
            for (subtree, _) in items {
                collect_globs(subtree, path.clone(), out);
            }
        }
        UseTreeKind::Simple(_) => {}
    }
}
