//! `perfectionist::arbitrary_source_item_ordering` — hold a module
//! body to one fixed sequence: `pub mod` declarations, then `pub use`
//! re-exports, then private imports and everything else.
//!
//! Module layout:
//!
//! - [`classify`] — which section an item belongs to, and how a
//!   diagnostic names it.
//!
//! The rule runs as a [`LateLintPass`] that **re-parses each of the
//! crate's module source files** via [`crate::module_reparse`]. A
//! pre-expansion pass would leave out-of-line `mod foo;` modules
//! `ModKind::Unloaded` (their files are not read until macro
//! expansion), so it would silently skip every separate-file
//! submodule. Re-parsing reaches every module-scoped submodule while
//! keeping `#[cfg(...)]` gates intact (parsing does not strip cfg,
//! unlike the post-expansion AST). The sibling
//! `import_grouping_mismatch` rule shares the same machinery, down to
//! the `live_module_spans` guard on descending into an inline
//! `mod { ... }`.

use crate::common::DefaultState;
use crate::enclosing_hir::find_enclosing_hir_ids;
use crate::module_reparse::{SpanRange, parse_crate_module_files};
use crate::rule_index::{Register, rule};
use classify::Category;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::{Item, ItemKind, ModKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, Pos, Span};
use std::collections::HashSet;

mod classify;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Holds the top-level items of a module body to one fixed
    /// sequence:
    ///
    /// 1. `pub mod` declarations,
    /// 2. `pub use` re-exports,
    /// 3. private imports and every other item.
    ///
    /// An item that sits below a section it belongs above is flagged —
    /// a `pub mod` under a `pub use`, a `pub use` under a private
    /// import or a `fn`, and so on. The last section is not ordered
    /// internally: a private `use` and a `struct` may appear in either
    /// order.
    ///
    /// Two shapes are exempt, being neither flagged nor closing the
    /// section they sit in: an `extern crate` declaration, which
    /// `#[macro_use] extern crate foo;` pins to the top of a crate
    /// root, and a `#[cfg(...)]`-gated `use`, so a trailing
    /// `#[cfg(unix)] use ...;` block may follow the main import block.
    ///
    /// Only the top-level sequence of a module body is considered.
    /// Items nested deeper — inside an `impl`, a trait, or a function
    /// body — are out of scope.
    ///
    /// Any explicit visibility counts as `pub` for the ordering, so
    /// `pub(crate) mod` is a `pub mod` declaration and `pub(super) use`
    /// is a `pub use` re-export.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Rust
    /// attaches no meaning to the order of items in a module body, and
    /// no layout is wrong in the abstract. The one this rule enforces
    /// answers a reader's questions in the order they ask them: what
    /// this module *contains* (its submodules), what it *publishes*
    /// (its re-exports), and only then what it *borrows* and how it is
    /// built. Applying it uniformly also keeps the top of every file
    /// scanning alike, and stops a new submodule from being appended
    /// wherever the diff happened to be open.
    ///
    /// No autofix is offered. Moving an item carries its attributes,
    /// doc comment, and any comment written above it, and a `pub use`
    /// hoisted above a `pub mod` may cross an item it names — edits a
    /// reader should make deliberately.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::arbitrary_source_item_ordering` (`restriction`, off by
    /// default) covers adjacent ground from the other end. It is highly
    /// general: it orders many item kinds against an ordering table the
    /// project supplies and can sort alphabetically within a group.
    /// This rule takes no configuration and bakes in the single layout
    /// above. A project that wants to spell out its own ordering should
    /// prefer the Clippy lint and turn this one off in `dylint.toml`:
    ///
    /// ```toml
    /// [perfectionist]
    /// disable = ["arbitrary_source_item_ordering"]
    /// ```
    ///
    /// Enabling both lets the two orderings contradict each other.
    ///
    /// ### Example
    ///
    /// **Avoid:** a `pub mod` below the `pub use` that re-exports from it
    ///
    /// ```rust,ignore
    /// pub use parser::Parser;
    /// pub mod parser;
    /// ```
    ///
    /// **Avoid:** a private `fn` above a `pub use`
    ///
    /// ```rust,ignore
    /// fn helper() {}
    /// pub use parser::Parser;
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// pub mod parser;
    /// pub mod printer;
    ///
    /// pub use parser::Parser;
    /// pub use printer::Printer;
    ///
    /// use std::collections::HashMap;
    ///
    /// fn helper() {}
    /// ```
    pub perfectionist::ARBITRARY_SOURCE_ITEM_ORDERING,
    Warn,
    "item in a module body sits below a section it belongs above: `pub mod`, then `pub use`, then private imports and other items",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::arbitrary_source_item_ordering";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct ArbitrarySourceItemOrdering;

impl_lint_pass!(ArbitrarySourceItemOrdering => [ARBITRARY_SOURCE_ITEM_ORDERING]);

impl Register for rule::ArbitrarySourceItemOrdering {
    /// Active by default. The rule encodes one baked-in layout rather
    /// than a direction a project has to choose between, and its
    /// trigger reads only what the author wrote, so there is nothing
    /// to configure and no shape it guesses at.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[ARBITRARY_SOURCE_ITEM_ORDERING]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Late pass: out-of-line `mod foo;` modules are `ModKind::Unloaded`
        // until macro expansion, so a pre-expansion pass never sees them.
        // `check_crate` re-parses each source file instead (see the module
        // docs), reaching every module-scoped submodule while keeping
        // `#[cfg(...)]` gates intact.
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(ArbitrarySourceItemOrdering)));
    }
}

/// A detected violation parked until its enclosing HIR node is known.
/// The rule discovers violations by re-parsing source files, outside
/// the HIR walk, so emission is deferred and routed through
/// [`span_lint_hir_and_then`] at the enclosing node — that is what lets
/// a per-module / per-item `#[allow]` / `#[expect]` resolve, instead of
/// only a crate-root one.
struct Pending {
    /// Resolves the lint-level anchor: the offending item's own span,
    /// always contained by its HIR node. For an out-of-line `mod foo;`
    /// that is the declaration in the parent file — the module body
    /// being checked — so the anchor stays in the file that holds the
    /// violation.
    ///
    /// No proc-macro guard rides along. The rule reads source the
    /// author wrote: its items come from re-parsing the crate's own
    /// module files, which excludes proc-macro-synthesised modules, and
    /// an item carrying an expansion is dropped when it is classified.
    anchor: Span,
    /// The span the diagnostic points at: the offending item's opening
    /// line. What the rule flags is where an item sits, so its body
    /// adds nothing to the diagnostic and would bury a misplaced
    /// `pub mod` under the module it declares.
    span: Span,
    /// The opening line of the earlier item whose section the offender
    /// belongs above — the first one to reach the highest rank seen so
    /// far.
    blocker_span: Span,
    offender: Category,
    blocker: Category,
}

impl<'tcx> LateLintPass<'tcx> for ArbitrarySourceItemOrdering {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        // Re-parse every module source file (reaching out-of-line
        // submodules while keeping `#[cfg(...)]` gates intact) and check
        // each file's module bodies in turn. See [`crate::module_reparse`].
        let (crates, live_module_spans) = parse_crate_module_files(cx);
        let mut violations: Vec<Pending> = Vec::new();
        let source_map = cx.sess().source_map();
        for krate in &crates {
            check_items(
                source_map,
                &krate.items,
                &live_module_spans,
                &mut violations,
            );
        }
        if violations.is_empty() {
            return;
        }

        // Anchor each violation at its enclosing HIR node so a per-module
        // / per-item `#[allow]` resolves (emitting from `check_crate`
        // alone would sit at the crate root).
        let anchors: Vec<Span> = violations.iter().map(|pending| pending.anchor).collect();
        let hir_ids = find_enclosing_hir_ids(cx.tcx, &anchors);
        for (pending, hir_id) in violations.into_iter().zip(hir_ids) {
            let Pending {
                span,
                blocker_span,
                offender,
                blocker,
                ..
            } = pending;
            span_lint_hir_and_then(
                cx,
                ARBITRARY_SOURCE_ITEM_ORDERING,
                hir_id,
                span,
                format!(
                    "{} appears after a {}",
                    offender.subject(),
                    blocker.subject(),
                ),
                |diagnostic| {
                    diagnostic.span_note(
                        blocker_span,
                        format!("this {} belongs below it", blocker.subject()),
                    );
                    diagnostic.help(
                        "a module body is ordered `pub mod` declarations, then `pub use` \
                         re-exports, then private imports and other items",
                    );
                },
            );
        }
    }
}

/// Check one module body's top-level sequence, then descend into the
/// inline `mod { ... }` bodies it holds.
///
/// The check is a high-water mark over [`Category::rank`]: an item
/// whose rank falls below the highest rank already seen sits below a
/// section it belongs above. A violating item does not itself raise the
/// mark, so a run of misplaced `pub mod` declarations is reported once
/// each against the same earlier item rather than cascading.
fn check_items(
    source_map: &SourceMap,
    items: &[Box<Item>],
    live_module_spans: &HashSet<SpanRange>,
    violations: &mut Vec<Pending>,
) {
    // The highest-ranked category seen so far, paired with the span of
    // the first item to reach it — the earliest item the offender has
    // to be moved above, and so the one worth pointing at.
    let mut highest: Option<(Category, Span)> = None;
    for item in items {
        let Some(category) = classify::classify(item) else {
            continue;
        };
        match highest {
            Some((blocker, blocker_span)) if category.rank() < blocker.rank() => {
                violations.push(Pending {
                    anchor: item.span,
                    span: opening_line(source_map, item.span),
                    blocker_span: opening_line(source_map, blocker_span),
                    offender: category,
                    blocker,
                });
            }
            Some((blocker, _)) if category.rank() > blocker.rank() => {
                highest = Some((category, item.span));
            }
            // Equal rank leaves the mark on the earlier item; `None` is
            // the first classified item of the body.
            Some(_) => {}
            None => highest = Some((category, item.span)),
        }
    }

    // Descend into inline `mod { ... }` bodies, but only those that
    // survived `#[cfg(...)]`-stripping to the compiled crate. The
    // re-parse keeps cfg-disabled modules (parsing does not strip cfg),
    // so without this guard a `#[cfg(test)] mod tests { ... }` excluded
    // from a non-test build would be linted — and, having no HIR node,
    // could not be suppressed by a local `#[allow]`. Out-of-line
    // `mod foo;` modules are `ModKind::Unloaded` here; their files are
    // re-parsed in their own right by `check_crate` (and a cfg-disabled
    // `mod foo;` is never loaded, so its file never enters the source
    // map).
    for item in items {
        if let ItemKind::Mod(_, _, ModKind::Loaded(items, _, mod_spans)) = &item.kind
            && live_module_spans.contains(&(mod_spans.inner_span.lo(), mod_spans.inner_span.hi()))
        {
            check_items(source_map, items, live_module_spans, violations);
        }
    }
}

/// `span` truncated to its opening line, with trailing whitespace
/// dropped. An item's span runs to the end of its body, which for an
/// inline `mod foo { ... }`, a `struct`, or a `fn` is many lines of
/// source that say nothing about where the item sits. A snippet that
/// cannot be recovered (never expected: every span here comes from a
/// re-parsed file the source map already holds) leaves the span as it
/// is.
fn opening_line(source_map: &SourceMap, span: Span) -> Span {
    let Ok(snippet) = source_map.span_to_snippet(span) else {
        return span;
    };
    let line_end = snippet.find('\n').unwrap_or(snippet.len());
    let width = snippet[..line_end].trim_end().len();
    if width == snippet.len() {
        return span;
    }
    span.with_hi(span.lo() + BytePos::from_usize(width))
}
