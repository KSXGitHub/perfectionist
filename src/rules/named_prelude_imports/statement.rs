//! The `use` statement currently being walked, and the rewrite that
//! re-points its cherry-picked leaves at their canonical modules.
//!
//! `rustc_ast_lowering` desugars `use foo::{A, B};` into one HIR item
//! per leaf, so a leaf's own path span covers just its name inside the
//! braces. That span cannot be rewritten in place: the `foo::` prefix
//! is shared with the other leaves, which need not agree on a canonical
//! module, and a nested `use` entry may not start at `crate` or
//! `super`. The rewrite therefore rebuilds the statement's use tree
//! from the leaves.
//!
//! Where that rebuilt tree goes depends on the shape lowering left
//! behind. A single import keeps a path span covering its whole path,
//! so the tree replaces just that — and the `as` rename, which sits
//! outside it, survives untouched. A brace list keeps no span for its
//! tree at all (the head's path span stops at the shared prefix), so
//! the statement is re-rendered whole, carrying its visibility across.
//! Either span starts no earlier than the visibility, so a statement's
//! attributes are outside the rewrite and need no reproducing — which
//! matters most for a `#[cfg(...)]`, since cfg-stripping means a
//! lowered item no longer records that it had one.
//!
//! [`Statement`] accumulates the leaves as the pass walks them and
//! [`Statement::rewrite`] renders the replacement.

use super::canonical::Canonical;
use rustc_errors::Applicability;
use rustc_hir::{HirId, Item, PathSegment, UseKind};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{Ident, Span, kw};

/// One leaf of a `use` statement — a single named import or a glob.
pub(super) struct Leaf {
    /// The leaf item's own `HirId`: the diagnostic's owner, and what
    /// resolves a per-item or per-module `#[allow]` / `#[expect]`.
    pub(super) hir_id: HirId,
    /// The leaf's written path span — where the diagnostic points.
    pub(super) path_span: Span,
    /// The leaf's path as written, rebuilt from its HIR segments
    /// (`crate::prelude::A`, `crate::prelude::*`).
    pub(super) written: String,
    /// The `as` rename's rendered suffix (`" as Alias"`), empty when the
    /// leaf is not renamed.
    pub(super) rename: String,
    /// A `self` entry (`use foo::prelude::{self, A}`). Rebuilding the
    /// tree around a different prefix turns it into a bare module
    /// import, which binds the name in every namespace rather than just
    /// the module — the same namespace hazard
    /// `import_granularity_mismatch` weighs for its `self_merge` knob —
    /// so a statement holding one is never rewritten.
    pub(super) is_self: bool,
    /// Set when this leaf cherry-picks a named item out of a prelude.
    pub(super) flagged: Option<Canonical>,
}

/// The `use` statement being walked: the span a rewrite replaces, plus
/// every leaf seen so far.
pub(super) struct Statement {
    /// The head item's own span: the whole `pub use ...;`, starting at
    /// the visibility and stopping at the `;`. It contains every leaf's
    /// span, which is how the pass tells where one statement's leaves
    /// end and the next statement begins, and it is what a brace list's
    /// rewrite replaces.
    pub(super) span: Span,
    /// The head's written path span. For a single import this covers
    /// the whole path and is what the rewrite replaces; for a brace list
    /// it stops at the prefix shared by the entries, which is why such a
    /// statement is re-rendered whole instead.
    path_span: Span,
    /// The head item's visibility span, empty for a private `use`.
    /// Reproduced when the whole statement is re-rendered.
    vis_span: Span,
    /// Whether the head is a brace list, whose entries carry their own
    /// `as` renames inside the replaced span.
    braced: bool,
    pub(super) leaves: Vec<Leaf>,
}

impl Statement {
    /// Start accumulating the statement headed by `item` — the outer
    /// `use` item, which for a brace list is the `ListStem` whose spans
    /// cover every leaf.
    pub(super) fn new(item: &Item<'_>, path_span: Span, kind: UseKind) -> Self {
        Statement {
            span: item.span,
            path_span,
            vis_span: item.vis_span,
            braced: matches!(kind, UseKind::ListStem),
            leaves: Vec::new(),
        }
    }

    /// The rewrite that re-points the statement's cherry-picks at their
    /// canonical modules, or `None` when it cannot be rewritten
    /// mechanically.
    pub(super) fn rewrite(&self, cx: &LateContext<'_>) -> Option<Fix> {
        // A `self` entry changes what it binds when the tree is rebuilt
        // around it, and a leaf whose path did not render has nothing to
        // put in the tree.
        if self
            .leaves
            .iter()
            .any(|leaf| leaf.is_self || leaf.written.is_empty())
        {
            return None;
        }
        let mut entries = Vec::with_capacity(self.leaves.len());
        for leaf in &self.leaves {
            let path = match &leaf.flagged {
                // A cherry-pick with no single canonical path (a name
                // binding items in several modules at once) cannot be
                // re-pointed, and leaving it on the prelude path would
                // render a "fix" the rule flags again.
                Some(canonical) => canonical.path.as_deref()?,
                None => leaf.written.as_str(),
            };
            // Outside a brace list the statement has exactly one leaf,
            // and the span being replaced stops before its `as` rename —
            // so rendering the rename here would duplicate it.
            let rename = if self.braced {
                leaf.rename.as_str()
            } else {
                ""
            };
            entries.push((path, rename));
        }

        let source_map = cx.sess().source_map();
        let tree = render_tree(&entries);
        let (span, replacement) = if self.braced {
            // A brace list's written tree has no span of its own in HIR,
            // so the statement is re-rendered from its visibility to its
            // `;`.
            let vis = source_map.span_to_snippet(self.vis_span).ok()?;
            let vis = if vis.is_empty() {
                vis
            } else {
                format!("{vis} ")
            };
            (self.span, format!("{vis}use {tree};"))
        } else {
            (self.path_span, tree)
        };

        // Down to `MaybeIncorrect` when the rewrite cannot be trusted
        // wholesale: a canonical module that is not `pub` all the way to
        // the crate root is not nameable from every importer, and a
        // comment written inside the replaced span is dropped by it.
        let nameable = self
            .leaves
            .iter()
            .all(|leaf| leaf.flagged.as_ref().is_none_or(|c| c.nameable));
        let snippet = source_map.span_to_snippet(span).ok()?;
        let commented = snippet.contains("//") || snippet.contains("/*");
        Some(Fix {
            span,
            replacement,
            applicability: super::applicability(nameable && !commented),
            label: if self.braced {
                "import each item from its canonical module"
            } else {
                "import the item from its canonical module"
            },
        })
    }
}

/// One offered rewrite: what to replace, with what, and under which
/// heading the diagnostic presents it.
pub(super) struct Fix {
    pub(super) span: Span,
    pub(super) replacement: String,
    pub(super) applicability: Applicability,
    pub(super) label: &'static str,
}

/// Render `entries` — each a `(path, rename suffix)` pair — as one
/// `use` tree, factoring out the segments every path shares.
///
/// Folding the shared prefix back out is what keeps the rewrite of
/// `use diesel::prelude::{table, AsChangeset};` down to
/// `diesel::{table, AsChangeset}` instead of spelling `diesel::` twice.
/// Entries stay in written order: which entry goes where is
/// `import_granularity_mismatch`'s business, not this rule's, and
/// reordering here would fight whatever that rule is configured to
/// enforce.
fn render_tree(entries: &[(&str, &str)]) -> String {
    let paths: Vec<Vec<&str>> = entries
        .iter()
        .map(|(path, _)| path.split("::").collect())
        .collect();
    // Every entry has to keep at least its own last segment, so the
    // shared prefix can never swallow a whole path.
    let limit = paths
        .iter()
        .map(|segments| segments.len().saturating_sub(1))
        .min()
        .unwrap_or(0);
    let mut shared = 0;
    while shared < limit
        && paths
            .iter()
            .all(|segments| segments[shared] == paths[0][shared])
    {
        shared += 1;
    }
    let tails: Vec<String> = paths
        .iter()
        .zip(entries)
        .map(|(segments, (_, rename))| format!("{}{rename}", segments[shared..].join("::")))
        .collect();
    // A lone entry needs no braces; several do.
    let body = match <[String; 1]>::try_from(tails) {
        Ok([single]) => single,
        Err(tails) => format!("{{{}}}", tails.join(", ")),
    };
    if shared == 0 {
        body
    } else {
        format!("{}::{body}", paths[0][..shared].join("::"))
    }
}

/// A `use` path's segments as they were written
/// (`["crate", "prelude", "A"]` → `"crate::prelude::A"`).
///
/// A leading `::` shows up in the HIR path as a synthetic `PathRoot`
/// segment and is rendered back as the `::` it was, so rewriting
/// `use ::serde::prelude::{A, B};` keeps every untouched leaf rooted at
/// the extern crate rather than silently re-resolving it against a
/// local `serde`. Each name goes through an [`Ident`] so a keyword
/// module name round-trips as the raw identifier (`r#type`) that
/// parses.
pub(super) fn written_path(segments: &[PathSegment<'_>]) -> String {
    let mut rendered = String::new();
    for (index, segment) in segments.iter().enumerate() {
        if segment.ident.name == kw::PathRoot {
            // `PathRoot` is only ever a leading `::`; anywhere else it
            // is not something a path can be written with.
            if index == 0 {
                rendered.push_str("::");
            }
            continue;
        }
        if !rendered.is_empty() && !rendered.ends_with("::") {
            rendered.push_str("::");
        }
        rendered.push_str(&Ident::with_dummy_span(segment.ident.name).to_string());
    }
    rendered
}

/// The `" as Alias"` suffix a leaf needs, or the empty string when the
/// binding is just the path's last segment under its own name.
pub(super) fn rename_suffix(segments: &[PathSegment<'_>], binding: Ident) -> String {
    if segments.last().map(|segment| segment.ident.name) == Some(binding.name) {
        String::new()
    } else {
        format!(" as {binding}")
    }
}

/// Whether a written `use` entry names the enclosing module itself —
/// the `self` of `use foo::{self, Bar}` or of `use foo::self;`.
///
/// Lowering pops that `self` off the path and rebinds the entry to the
/// module, so the HIR segments no longer say which of the two shapes
/// was written; the source text does.
pub(super) fn is_self_entry(written: &str) -> bool {
    written
        .rsplit("::")
        .next()
        .is_some_and(|last| last.trim() == "self")
}

#[cfg(test)]
mod tests;
