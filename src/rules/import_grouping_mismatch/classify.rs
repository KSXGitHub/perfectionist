//! Classifying one `use` statement into a group rank.
//!
//! The rank is what the rest of the rule compares: a smaller rank sorts
//! earlier. Path classification looks at the *first segment* of the
//! statement's path; a `#[cfg(...)]`-gated import is either slotted by
//! that same path (under [`CfgBlockHandling::Merge`]) or hoisted into a
//! single trailing group (under [`CfgBlockHandling::Trailing`]).

use super::config::{CfgBlockHandling, Config, Group, ReexportGrouping, Style};
use rustc_ast::{UseTree, UseTreeKind};
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

/// Classify a statement's path into one of the three groups by its
/// first segment. A path with no leading segment — a top-level brace
/// spanning several crate roots, or a global `::*` — has no single crate
/// root to key on and falls into `thirdparty`, the catch-all.
///
/// `local_modules` holds the names of `mod` items declared in the same
/// module scope as this `use`. A bare first segment naming one of them
/// is an import of a first-party submodule (`mod error; use error::Foo;`)
/// — in editions 2018+ a local item shadows the extern prelude, so the
/// bare path resolves to the local module, not a same-named crate — so
/// it is classified `internal` rather than falling through to the
/// `thirdparty` catch-all. The rule reads source syntactically, without
/// name resolution, so this sibling-`mod` match is the syntactic
/// approximation of that resolution. The built-in std and internal
/// segment names are matched first, so a sibling `mod` sharing one of
/// those names keeps its built-in group.
fn path_group(tree: &UseTree, local_modules: &HashSet<String>) -> Group {
    let Some(first) = first_segment(tree) else {
        return Group::Thirdparty;
    };
    match first.as_str() {
        "std" | "core" | "alloc" | "proc_macro" | "test" => Group::Std,
        "crate" | "super" | "self" => Group::Internal,
        _ if local_modules.contains(&first) => Group::Internal,
        _ => Group::Thirdparty,
    }
}

/// Whether a re-export is an *alias* re-export rather than a *submodule*
/// re-export, the split [`ReexportGrouping::Split`] draws. An alias
/// re-export is a single-segment simple path — `pub use Item;` or
/// `pub use Item as Alias;` — that only renames an item already in scope;
/// a submodule re-export carries a `::` (`pub use child::Item;`,
/// `pub use child::{A, B};`, `pub use child::*;`) and lifts an item out of
/// a child module. The test is purely syntactic: a `Simple` tree whose
/// written prefix is one segment (ignoring a leading `::`). A nested or
/// glob tree always carries a `::`, and a top-level brace
/// (`pub use {a, b};`) has no single segment, so both count as submodule
/// re-exports. A `pub use foo;` re-exporting an external crate is a
/// single segment and so classifies as an alias re-export.
fn is_alias_reexport(tree: &UseTree) -> bool {
    matches!(tree.kind, UseTreeKind::Simple(_))
        && tree
            .prefix
            .segments
            .iter()
            .filter(|segment| segment.ident.name != kw::PathRoot)
            .count()
            == 1
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
///
/// `local_modules` is forwarded to [`path_group`] to recognise bare
/// imports of sibling `mod`s. It is consulted only on the `multi_block`
/// path: `single_block` ignores path origin, so a sibling-`mod` import
/// is never distinguished there.
///
/// When `reexports` separates re-exports, a re-export (`is_reexport`)
/// takes a dedicated leading rank regardless of style, path, or cfg
/// gating — visibility outranks every other partition — and every
/// non-re-export rank shifts down past it
/// ([`Config::reexport_block_offset`]):
///
/// - `grouped` gives every re-export the single leading rank `0`, one
///   block above the styled private-import blocks.
/// - `split` keeps submodule re-exports at rank `0` and alias re-exports
///   at rank `1`, two blocks above the private imports — see
///   [`is_alias_reexport`].
/// - `by_path` reserves no leading rank; a re-export is classified by its
///   path exactly like a private import.
pub(super) fn rank(
    tree: &UseTree,
    is_reexport: bool,
    is_cfg_gated: bool,
    config: &Config,
    local_modules: &HashSet<String>,
) -> usize {
    if is_reexport {
        match config.reexports {
            ReexportGrouping::ByPath => {}
            ReexportGrouping::Grouped => return 0,
            ReexportGrouping::Split => return usize::from(is_alias_reexport(tree)),
        }
    }
    let cfg_trailing =
        is_cfg_gated && matches!(config.cfg_block_handling, CfgBlockHandling::Trailing);
    let base = match config.style {
        Style::SingleBlock => usize::from(cfg_trailing),
        Style::MultiBlock if cfg_trailing => config.cfg_rank(),
        Style::MultiBlock => config.group_rank(path_group(tree, local_modules)),
    };
    // Shift the private-import ranks down past the leading re-export
    // region so its ranks (`0`, or `0` and `1` under `split`) stay
    // reserved for re-exports.
    base + config.reexport_block_offset()
}
