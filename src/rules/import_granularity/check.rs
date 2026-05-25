//! Per-style compliance predicates. Each takes the flattened
//! [`StmtInfo`]s of one compatible group of consecutive `use`
//! statements and answers "is this group already in the configured
//! shape?". A `false` answer drives [`super::render`] to produce the
//! canonical replacement.

use std::collections::HashSet;

use super::config::Style;
use super::model::{StmtInfo, TopKind};

pub(super) fn is_compliant(style: Style, stmts: &[&StmtInfo]) -> bool {
    match style {
        Style::Item => stmts.iter().all(|stmt| is_item_shaped(stmt)),
        Style::Module => module_compliant(stmts),
        Style::Crate => crate_compliant(stmts),
    }
}

/// `item` style: every statement imports exactly one flat leaf, so its
/// top-level tree is a plain path (`Simple`) or a glob (`Glob`). Any
/// brace group (`Nested`) carries more than one leaf — or redundant
/// braces around one — and must be split.
fn is_item_shaped(stmt: &StmtInfo) -> bool {
    !matches!(stmt.top_kind, TopKind::Nested)
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
        // the same module and that module is the written prefix.
        TopKind::Nested => stmt
            .common_module()
            .is_some_and(|module| module == stmt.prefix.as_slice()),
    }
}

/// `crate` style: every statement is maximally collapsed, descends from
/// a single crate root, and no two statements share a root.
fn crate_compliant(stmts: &[&StmtInfo]) -> bool {
    if !stmts.iter().all(|stmt| stmt.collapsed) {
        return false;
    }
    // Unlike `module` style, crate items participate here: `use foo;`
    // and `use foo::Bar;` share the root `foo` and must collapse to
    // `use foo::{self, Bar};`.
    let mut seen: HashSet<&str> = HashSet::new();
    for stmt in stmts {
        // A top-level brace spanning several roots (`use {a::X, b::Y};`)
        // has no single root and is never crate-shaped.
        let Some(root) = stmt.crate_root() else {
            return false;
        };
        if !seen.insert(root) {
            return false;
        }
    }
    true
}
