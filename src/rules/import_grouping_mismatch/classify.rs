//! Classifying one `use` statement into a group rank.
//!
//! The rank is what the rest of the rule compares: a smaller rank sorts
//! earlier. Path classification looks at the *first segment* of the
//! statement's path; a `#[cfg(...)]`-gated import is either slotted by
//! that same path (under [`CfgBlockHandling::Merge`]) or hoisted into a
//! single trailing group (under [`CfgBlockHandling::Trailing`]).

use super::config::{CfgBlockHandling, Config, Group};
use rustc_ast::UseTree;
use rustc_span::kw;
use std::collections::HashSet;

/// The first path segment of a `use` tree's written prefix, skipping a
/// leading path-root (`::`). `None` for a top-level brace
/// (`use {a, b};`) whose prefix is empty, or a bare `use ::*;`.
fn first_segment(tree: &UseTree) -> Option<String> {
    for segment in &tree.prefix.segments {
        if segment.ident.name == kw::PathRoot {
            continue;
        }
        return Some(segment.ident.name.to_string());
    }
    None
}

/// Classify a statement's path into one of the three groups. A path
/// with no leading segment — a top-level brace spanning several crate
/// roots, or a global `::*` — has no single crate root to key on and
/// falls into `thirdparty`, the catch-all.
///
/// `local_modules` holds the names of `mod` items declared in the same
/// module scope as this `use`. A bare first segment naming one of them
/// is an import of a first-party submodule (`mod error; use error::Foo;`)
/// — in editions 2018+ a local item shadows the extern prelude, so the
/// bare path resolves to the local module, not a same-named crate — so
/// it is classified `internal` rather than falling through to the
/// `thirdparty` catch-all. The rule reads source syntactically, without
/// name resolution, so this sibling-`mod` match is the syntactic
/// approximation of that resolution. The configured `std_crates` and
/// `internal_prefixes` lists are explicit user intent and so are
/// honoured first.
fn path_group(tree: &UseTree, config: &Config, local_modules: &HashSet<String>) -> Group {
    let Some(first) = first_segment(tree) else {
        return Group::Thirdparty;
    };
    if config.std_crates.contains(&first) {
        Group::Std
    } else if config.internal_prefixes.contains(&first) || local_modules.contains(&first) {
        Group::Internal
    } else {
        Group::Thirdparty
    }
}

/// The rank a statement sorts by: its path group's position in the
/// configured order, except a cfg-gated import under
/// [`CfgBlockHandling::Trailing`], which takes the always-last cfg
/// rank regardless of its path. `local_modules` is forwarded to
/// [`path_group`] to recognise bare imports of sibling `mod`s.
pub(super) fn rank(
    tree: &UseTree,
    is_cfg_gated: bool,
    config: &Config,
    local_modules: &HashSet<String>,
) -> usize {
    if is_cfg_gated && matches!(config.cfg_block_handling, CfgBlockHandling::Trailing) {
        return config.cfg_rank();
    }
    config.group_rank(path_group(tree, config, local_modules))
}
