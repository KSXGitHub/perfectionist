//! Per-style compliance predicates. Each takes the flattened
//! [`StmtInfo`]s of one compatible group of consecutive `use`
//! statements and answers "is this group already in the configured
//! shape?". A `false` answer drives [`super::render`] to produce the
//! canonical replacement.

use std::collections::HashSet;

use super::config::{SelfMerge, Style};
use super::model::{LeafItem, StmtInfo, TopKind, has_bare_item_dual, has_self_dual};

pub(super) fn is_compliant(
    style: Style,
    self_merge: Option<SelfMerge>,
    stmts: &[&StmtInfo],
) -> bool {
    match style {
        Style::Item => stmts.iter().all(|stmt| is_item_shaped(stmt)),
        Style::Module => module_compliant(stmts),
        Style::Crate => crate_compliant(self_merge, stmts),
    }
}

/// `item` style: every statement imports exactly one flat leaf, so its
/// top-level tree is a plain path (`Simple`) or a glob (`Glob`). A brace
/// group (`Nested`) carries more than one leaf — or redundant braces —
/// and must be split. The one exception is the irreducible single
/// `self` (`use a::b::{self}`): it is one leaf, and `self` cannot be
/// written without braces, so no item-style rewrite can remove them.
fn is_item_shaped(stmt: &StmtInfo) -> bool {
    match stmt.top_kind {
        TopKind::Simple | TopKind::Glob => true,
        TopKind::Nested => {
            matches!(stmt.leaves.as_slice(), [leaf] if matches!(leaf.item, LeafItem::SelfMod))
        }
    }
}

/// `module` style: every statement's leaves come from a single module
/// whose path is exactly the written prefix (no items pushed down into
/// nested braces), and no two statements target the same module (which
/// would mean a module was split across lines).
fn module_compliant(stmts: &[&StmtInfo]) -> bool {
    if !stmts.iter().all(|stmt| is_module_shaped(stmt)) {
        return false;
    }
    let mut seen: HashSet<&[String]> = HashSet::new();
    for stmt in stmts {
        // Crate items (`use serde;`) all share the empty module but are
        // never merged, so they don't participate in this check.
        if stmt.is_crate_item() {
            continue;
        }
        let module = stmt.common_module().expect("shaped ⇒ single module");
        if !seen.insert(module) {
            return false;
        }
    }
    true
}

fn is_module_shaped(stmt: &StmtInfo) -> bool {
    match stmt.top_kind {
        // A single flat leaf is always its own module form.
        TopKind::Simple | TopKind::Glob => true,
        // A brace group is module-shaped only when every leaf lives in
        // the same module and that module is the written prefix. A
        // top-level brace (`use {a, b};`, empty prefix) groups distinct
        // crate roots that module style keeps on separate lines, so it
        // is never shaped.
        TopKind::Nested => {
            !stmt.prefix.is_empty()
                && stmt
                    .common_module()
                    .is_some_and(|module| module == stmt.prefix.as_slice())
        }
    }
}

/// `crate` style: every statement is maximally collapsed, descends from
/// a single crate root, and no two statements share a root — except a
/// root imported as a bare crate item, which can't be folded with its
/// own deeper imports without an unsafe synthesised `self` (see below).
fn crate_compliant(self_merge: Option<SelfMerge>, stmts: &[&StmtInfo]) -> bool {
    // With `self_merge` set, the project has picked one shape for a name
    // that is both an item and a module, so the opposite single-statement
    // form is no longer compliant: `fold` flags the sibling-split
    // (`{thing, thing::T}`) and `split` flags the `self`-fold
    // (`thing::{self, T}`). Unset, both shapes stay compliant (the safe
    // default). See
    // <https://github.com/KSXGitHub/perfectionist/issues/206>.
    let off_shape = match self_merge {
        Some(SelfMerge::Fold) => stmts.iter().any(|stmt| has_bare_item_dual(&stmt.leaves)),
        Some(SelfMerge::Split) => stmts.iter().any(|stmt| has_self_dual(&stmt.leaves)),
        None => false,
    };
    if off_shape {
        return false;
    }
    if !stmts.iter().all(|stmt| stmt.collapsed) {
        return false;
    }
    // A root imported as a bare crate item (`use foo;`) can't be folded
    // together with a deeper import from the same root (`use foo::Bar;`):
    // the only one-statement form is `use foo::{self, Bar};`, whose
    // `self` re-imports the *module* `foo` alone, silently dropping any
    // value or macro named `foo` and breaking the code that used it.
    // `use foo;` binds `foo` in every namespace it exists in, so it is
    // not interchangeable with the module-only `self`. There is no safe
    // one-`use`-per-root shape, so such a root is left split across
    // statements rather than flagged — the same reasoning that keeps
    // `is_collapsed` from folding `{thing, thing::Opts}` inside a single
    // brace. See <https://github.com/KSXGitHub/perfectionist/issues/186>.
    let bare_item_roots: HashSet<&str> = stmts
        .iter()
        .filter(|stmt| stmt.is_crate_item())
        .filter_map(|stmt| stmt.crate_root())
        .collect();
    let mut seen: HashSet<&str> = HashSet::new();
    for stmt in stmts {
        // A top-level brace spanning several roots (`use {a::X, b::Y};`)
        // has no single root and is never crate-shaped.
        let Some(root) = stmt.crate_root() else {
            return false;
        };
        if bare_item_roots.contains(root) {
            continue;
        }
        if !seen.insert(root) {
            return false;
        }
    }
    true
}
