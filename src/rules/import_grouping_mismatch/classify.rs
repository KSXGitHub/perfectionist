//! Classifying one `use` statement into a group rank.
//!
//! The rank is what the rest of the rule compares: a smaller rank sorts
//! earlier. Path classification looks at the *first segment* of the
//! statement's path; a `#[cfg(...)]`-gated import is either slotted by
//! that same path (under [`CfgBlockHandling::Merge`]) or hoisted into a
//! single trailing group (under [`CfgBlockHandling::Trailing`]).

use super::config::{CfgBlockHandling, Config, Group, Style};
use rustc_ast::UseTree;
use rustc_span::kw;

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

/// Classify a statement's path into one of the three groups by its
/// first segment. A path with no leading segment — a top-level brace
/// spanning several crate roots, or a global `::*` — has no single crate
/// root to key on and falls into `thirdparty`, the catch-all.
fn path_group(tree: &UseTree) -> Group {
    let Some(first) = first_segment(tree) else {
        return Group::Thirdparty;
    };
    match first.as_str() {
        "std" | "core" | "alloc" | "proc_macro" | "test" => Group::Std,
        "crate" | "super" | "self" => Group::Internal,
        _ => Group::Thirdparty,
    }
}

/// The rank a statement sorts by. The style decides the partition;
/// under both, a cfg-gated import is hoisted into a trailing block only
/// when `cfg_block_handling` is [`CfgBlockHandling::Trailing`]:
///
/// - `single_block` keeps every import in one block (rank `0`), so the
///   run admits no blank lines — except a trailing cfg import, which
///   takes a higher rank `1` and forms a single trailing block. Path
///   origin is irrelevant here.
/// - `multi_block` ranks by the path group's position in the configured
///   order, except a trailing cfg import, which takes the always-last
///   cfg rank regardless of its path.
pub(super) fn rank(tree: &UseTree, is_cfg_gated: bool, config: &Config) -> usize {
    let cfg_trailing =
        is_cfg_gated && matches!(config.cfg_block_handling, CfgBlockHandling::Trailing);
    match config.style {
        Style::SingleBlock => usize::from(cfg_trailing),
        Style::MultiBlock if cfg_trailing => config.cfg_rank(),
        Style::MultiBlock => config.group_rank(path_group(tree)),
    }
}
