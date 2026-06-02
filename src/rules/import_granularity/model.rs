//! Flattening a `use` tree into its leaves, plus the structural facts
//! the compliance checks ([`super::check`]) and the canonical renderer
//! ([`super::render`]) read off each statement.

use rustc_ast::{UseTree, UseTreeKind};
use rustc_span::kw;

/// The thing imported at a leaf of a `use` tree.
#[derive(Clone, PartialEq, Eq)]
pub(super) enum LeafItem {
    /// A named item — the `HashMap` of `use std::collections::HashMap`.
    Named(String),
    /// A glob — the `*` of `use std::collections::*`.
    Glob,
    /// A `self` import — the `self` of `use foo::bar::{self, Bar}`.
    SelfMod,
}

/// One imported leaf, flattened out of a (possibly nested) `use` tree.
#[derive(Clone)]
pub(super) struct Leaf {
    /// Segments of the module the item lives in. `use std::io::Read`
    /// gives `["std", "io"]`; the glob `use std::io::*` also gives
    /// `["std", "io"]`. A bare single-segment `use serde;` gives `[]`.
    pub(super) module: Vec<String>,
    pub(super) item: LeafItem,
    /// The `as` rename, if any (`use a::b as c` → `Some("c")`).
    pub(super) rename: Option<String>,
}

impl Leaf {
    /// A "crate item" is a bare single-segment import like `use serde;`
    /// or `use serde as alias;`. It has no module path, can't be split
    /// or merged any further, and so is left standalone under every
    /// style.
    pub(super) fn is_crate_item(&self) -> bool {
        self.module.is_empty() && matches!(self.item, LeafItem::Named(_))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TopKind {
    Simple,
    Glob,
    Nested,
}

/// Everything one `use` statement contributes to the group analysis.
pub(super) struct StmtInfo {
    pub(super) leaves: Vec<Leaf>,
    pub(super) top_kind: TopKind,
    /// Segments of the top-level tree's written prefix — the path
    /// before the final name (`Simple`), the `*` (`Glob`), or the brace
    /// group (`Nested`). Empty for a top-level brace `use {a, b};`.
    pub(super) prefix: Vec<String>,
    /// Whether the written tree is already maximally collapsed (no two
    /// brace entries share a leading segment, no redundant single-item
    /// braces). Used by the `crate` style.
    pub(super) collapsed: bool,
}

impl StmtInfo {
    pub(super) fn is_crate_item(&self) -> bool {
        self.leaves.len() == 1 && self.leaves[0].is_crate_item()
    }

    /// The module shared by every leaf, or `None` when the leaves span
    /// more than one module (a cross-module statement).
    pub(super) fn common_module(&self) -> Option<&[String]> {
        let first = self.leaves.first()?;
        self.leaves
            .iter()
            .all(|leaf| leaf.module == first.module)
            .then_some(first.module.as_slice())
    }

    /// The single crate root every leaf descends from, or `None` for a
    /// top-level brace whose leaves come from different roots.
    pub(super) fn crate_root(&self) -> Option<&str> {
        self.prefix.first().map(String::as_str)
    }
}

/// Whether `leaves` contains a bare named item that shares its name
/// with a deeper sibling — the `{thing, thing::T}` pattern. The `fold`
/// `self_merge` setting rewrites it to `thing::{self, T}`; that fold
/// re-imports only the *module* `thing`, silently dropping any value or
/// macro bound under the same name, so it is the lossy half of the
/// `fold` rewrite and is never applied without the opt-in. See
/// <https://github.com/KSXGitHub/perfectionist/issues/206>.
pub(super) fn has_bare_item_dual(leaves: &[Leaf]) -> bool {
    leaves.iter().enumerate().any(|(index, leaf)| {
        let LeafItem::Named(name) = &leaf.item else {
            return false;
        };
        let mut deeper = leaf.module.clone();
        deeper.push(name.clone());
        leaves
            .iter()
            .enumerate()
            .any(|(other, sibling)| other != index && sibling.module.starts_with(&deeper))
    })
}

/// Whether `leaves` contains a `self` import that has a deeper sibling —
/// the `thing::{self, T}` pattern. The `split` `self_merge` setting
/// rewrites it to `{thing, thing::T}`; raising the module-only `self` to
/// a bare item binds `thing` in *every* namespace, so it is the lossy
/// half of the `split` rewrite. A lone `use thing::{self}` has no deeper
/// sibling and is left alone. See
/// <https://github.com/KSXGitHub/perfectionist/issues/206>.
pub(super) fn has_self_dual(leaves: &[Leaf]) -> bool {
    leaves.iter().enumerate().any(|(index, leaf)| {
        matches!(leaf.item, LeafItem::SelfMod) && has_deeper_sibling(leaves, index)
    })
}

/// Whether some other leaf lives in the module named by `leaves[index]`
/// (or deeper) — i.e. `leaves[index]` is a `self`/module import with a
/// sibling under the same path. The leaf at `index` is excluded from the
/// comparison so it never counts as its own sibling.
pub(super) fn has_deeper_sibling(leaves: &[Leaf], index: usize) -> bool {
    let module = &leaves[index].module;
    leaves
        .iter()
        .enumerate()
        .any(|(other, sibling)| other != index && sibling.module.starts_with(module))
}

/// Build a [`StmtInfo`] for one top-level `use` tree, or `None` when the
/// statement contains something the rule declines to rewrite (a global
/// `::` path root, or a malformed `self`).
pub(super) fn stmt_info(tree: &UseTree) -> Option<StmtInfo> {
    let leaves = flatten(tree)?;
    let mut prefix = Vec::with_capacity(tree.prefix.segments.len());
    for segment in &tree.prefix.segments {
        if segment.ident.name == kw::PathRoot {
            return None;
        }
        prefix.push(segment.ident.name.to_string());
    }
    let top_kind = match tree.kind {
        UseTreeKind::Simple(_) => TopKind::Simple,
        UseTreeKind::Glob(_) => TopKind::Glob,
        UseTreeKind::Nested { .. } => TopKind::Nested,
    };
    Some(StmtInfo {
        leaves,
        top_kind,
        prefix,
        collapsed: is_collapsed(tree),
    })
}

fn flatten(tree: &UseTree) -> Option<Vec<Leaf>> {
    let mut out = Vec::new();
    flatten_into(tree, &[], &mut out)?;
    Some(out)
}

fn flatten_into(tree: &UseTree, prefix: &[String], out: &mut Vec<Leaf>) -> Option<()> {
    let mut combined = prefix.to_vec();
    for segment in &tree.prefix.segments {
        if segment.ident.name == kw::PathRoot {
            return None;
        }
        combined.push(segment.ident.name.to_string());
    }
    match &tree.kind {
        UseTreeKind::Simple(rename) => {
            let rename = rename.map(|ident| ident.name.to_string());
            let last = combined.last()?;
            if last == "self" {
                // `use a::b::self` / `{self}` imports the module `a::b`.
                combined.pop();
                if combined.is_empty() {
                    return None;
                }
                out.push(Leaf {
                    module: combined,
                    item: LeafItem::SelfMod,
                    rename,
                });
            } else {
                let name = combined.pop()?;
                out.push(Leaf {
                    module: combined,
                    item: LeafItem::Named(name),
                    rename,
                });
            }
        }
        UseTreeKind::Glob(_) => {
            // A glob with no module path (`use *;` / `use {*};`) has
            // nothing to anchor a rewrite onto; decline the statement.
            if combined.is_empty() {
                return None;
            }
            out.push(Leaf {
                module: combined,
                item: LeafItem::Glob,
                rename: None,
            });
        }
        UseTreeKind::Nested { items, .. } => {
            for (subtree, _) in items {
                flatten_into(subtree, &combined, out)?;
            }
        }
    }
    Some(())
}

/// Whether `tree` is already maximally collapsed: no brace level holds
/// two entries that share a leading segment *and* can be merged without
/// synthesising a `self`, and no brace wraps a single non-`self` entry.
/// This is the per-statement half of the `crate`-style check — a
/// collapsed statement with a unique crate root needs no fix.
fn is_collapsed(tree: &UseTree) -> bool {
    let UseTreeKind::Nested { items, .. } = &tree.kind else {
        return true;
    };
    if items.len() == 1 {
        // `use a::{b}` is redundant (→ `use a::b`); the lone exception
        // is `use a::{self}`, which cannot drop its braces.
        let (sub, _) = &items[0];
        return is_lone_self(sub) && is_collapsed(sub);
    }
    let firsts: Vec<_> = items
        .iter()
        .map(|(sub, _)| sub.prefix.segments.first().map(|seg| seg.ident.name))
        .collect();
    // Two or more entries that share a leading segment are mergeable —
    // and thus not collapsed — only when every one of them continues
    // past that segment (`{path::Path, path::PathBuf}` →
    // `path::{Path, PathBuf}`). When a *bare terminal* entry shares the
    // segment (`{thing, thing::Opts}`), the only collapse is
    // `thing::{self, Opts}`, and the synthesised `self` re-imports the
    // *module* `thing` alone — silently dropping any value or macro
    // re-exported under the same name, which stops the using code from
    // compiling. We can't tell from syntax whether such a name exists,
    // so the bare-terminal form is treated as already collapsed rather
    // than risk a build-breaking rewrite. See
    // <https://github.com/KSXGitHub/perfectionist/issues/186>. Two
    // terminal leaves that merely share a name (`{B, B as C}`, distinct
    // local bindings of the same path) likewise cannot be folded
    // further, so they stay collapsed. A bare glob (`*`) has no leading
    // segment, shares with nothing, and is already canonical, so it is
    // skipped.
    for index in 0..items.len() {
        let Some(first) = firsts[index] else {
            continue;
        };
        // Evaluate each distinct leading segment once, over the whole
        // brace level: skip any segment already handled at an earlier
        // index. A bare terminal *anywhere* in the group forces `self`,
        // so the group must be considered as a whole — a windowed scan
        // would wrongly flag `{thing::A, thing::B}` inside
        // `{thing, thing::A, thing::B}` while ignoring the bare `thing`.
        if firsts[..index].contains(&Some(first)) {
            continue;
        }
        let mut continuing = 0usize;
        let mut bare_terminal = false;
        for (other_index, other) in items.iter().enumerate() {
            if firsts[other_index] == Some(first) {
                if continues(&other.0) {
                    continuing += 1;
                } else {
                    bare_terminal = true;
                }
            }
        }
        // `continuing >= 2 && !bare_terminal` means two or more entries
        // share the segment and every one of them continues past it, so
        // they fold into `seg::{...}` with no synthesised `self`.
        if continuing >= 2 && !bare_terminal {
            return false;
        }
    }
    items.iter().all(|(sub, _)| is_collapsed(sub))
}

/// Whether a brace entry has structure past its leading segment — a
/// multi-segment path (`path::Path`), a nested group (`b::{...}`), or a
/// glob (`b::*`). A bare single-segment `Simple` (`B`, `B as C`) does
/// not.
fn continues(tree: &UseTree) -> bool {
    tree.prefix.segments.len() > 1 || !matches!(tree.kind, UseTreeKind::Simple(_))
}

fn is_lone_self(tree: &UseTree) -> bool {
    matches!(tree.kind, UseTreeKind::Simple(_))
        && tree.prefix.segments.len() == 1
        && tree.prefix.segments[0].ident.name == kw::SelfLower
}
