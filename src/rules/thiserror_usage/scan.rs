//! Crate-wide alias collection. A pre-expansion `EarlyLintPass`
//! cannot consult rustc's name resolver, so the rule reconstructs
//! what each `use thiserror::...` / `extern crate thiserror`
//! statement brought into scope by walking the AST itself. The two
//! alias maps it fills ([`ThiserrorUsage::aliases`]
//! and `crate_aliases`) let the detection pass recognise the bare
//! `#[derive(Error)]` and crate-renamed `#[derive(te::Error)]`
//! shorthands.

use super::config::{ThiserrorUsage, path_matches_thiserror};
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Crate, Item, ItemKind, UseTree, UseTreeKind};
use rustc_span::{Symbol, kw};

/// Populate `rule`'s alias maps from every `use` / `extern crate`
/// statement in `krate`.
///
/// Runs the AST visitor in two phases: the first fills
/// `crate_aliases` (crate renames), the second fills `aliases` (leaf
/// names), expanding crate-aliased first segments through the
/// already-complete `crate_aliases` map. Two passes make source
/// order irrelevant — a `use thiserror as te;` rename and a
/// dependent `use te::Error;` may appear in either order.
pub(super) fn collect_aliases(rule: &mut ThiserrorUsage, krate: &Crate) {
    // Reset state. dylint currently constructs a fresh pass per
    // crate, so the clear is defensive; a future host that reuses
    // instances would otherwise see aliases leak between crates.
    rule.aliases.clear();
    rule.crate_aliases.clear();
    for phase in [CollectPhase::CrateAliases, CollectPhase::LeafAliases] {
        let mut collector = AliasCollector {
            rule: &mut *rule,
            phase,
        };
        visit::walk_crate(&mut collector, krate);
    }
}

/// Phase tag for the two-pass alias collection. The first pass fills
/// `crate_aliases` from every `use thiserror as te;` /
/// `extern crate thiserror as te;` item; the second pass fills
/// `aliases`, expanding crate-aliased first segments
/// (`use te::Error;`) through `crate_aliases`.
#[derive(Clone, Copy)]
enum CollectPhase {
    CrateAliases,
    LeafAliases,
}

/// AST visitor driving the alias scan. Walking the AST through the
/// visitor (rather than a hand-rolled `ItemKind::Mod`-only recursion)
/// ensures that `use thiserror::...` and
/// `extern crate thiserror as ...;` items nested inside function
/// bodies, blocks, `impl`s, or trait items are recorded — the rule's
/// `check_item` callback still visits those inner items individually
/// for diagnostic emission, but the alias scan must happen up front
/// so that a bare `#[derive(Error)]` in any scope can be resolved
/// against a sibling `use thiserror::Error;` in any other scope.
struct AliasCollector<'a> {
    rule: &'a mut ThiserrorUsage,
    phase: CollectPhase,
}

impl<'ast> Visitor<'ast> for AliasCollector<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        match &item.kind {
            ItemKind::Use(use_tree) => {
                walk_use_tree_for_aliases(self.rule, use_tree, &[], self.phase);
            }
            ItemKind::ExternCrate(orig, ident) => {
                // `extern crate thiserror;` — `orig` is `None`,
                // `ident` is `thiserror`. `extern crate thiserror as te;`
                // — `orig` is `Some(thiserror)`, `ident` is `te`. Only
                // relevant during the crate-alias pass; the leaf-alias
                // pass relies on the already-populated map without
                // re-inserting.
                if matches!(self.phase, CollectPhase::CrateAliases) {
                    let original = orig.unwrap_or(ident.name);
                    if self.rule.thiserror_crates.contains(&original) {
                        self.rule.crate_aliases.insert(ident.name, original);
                    }
                }
            }
            _ => {}
        }
        visit::walk_item(self, item);
    }
}

fn walk_use_tree_for_aliases(
    rule: &mut ThiserrorUsage,
    tree: &UseTree,
    parent: &[Symbol],
    phase: CollectPhase,
) {
    let mut path: Vec<Symbol> = parent.to_vec();
    for (idx, segment) in tree.prefix.segments.iter().enumerate() {
        // `kw::PathRoot` is the synthetic leading-`::` segment; skip
        // it so `use ::thiserror::Error;` and `use thiserror::Error;`
        // produce the same `path`.
        if segment.ident.name == kw::PathRoot {
            continue;
        }
        // `kw::SelfLower` at position 0 of a *nested child's* prefix
        // (`use thiserror::{self};` / `use thiserror::{self as te};`)
        // names the parent itself — semantically equivalent to `use
        // thiserror;` / `use thiserror as te;`. Drop the segment so
        // the nested form joins the parent path and falls into the
        // single-segment crate-alias branch below.
        //
        // At the top level (`use self::Error;`) `self` is a real
        // module reference and must NOT be filtered, since
        // `self::Error` resolves through the current module rather
        // than the crate root. Gating on `!parent.is_empty()`
        // distinguishes the two positions.
        if !parent.is_empty() && idx == 0 && segment.ident.name == kw::SelfLower {
            continue;
        }
        path.push(segment.ident.name);
    }
    match &tree.kind {
        UseTreeKind::Simple(rename) => match phase {
            CollectPhase::CrateAliases => {
                // `use thiserror as te;` / `use thiserror;` —
                // single-segment path naming a thiserror crate root.
                // Record the local name as a crate-level alias so a
                // later `te::Error` derive (or `use te::Error;`) can
                // be expanded.
                if path.len() == 1 && rule.thiserror_crates.contains(&path[0]) {
                    let local = rename.map(|ident| ident.name).unwrap_or(path[0]);
                    rule.crate_aliases.insert(local, path[0]);
                }
            }
            CollectPhase::LeafAliases => {
                // `use thiserror::Error;` / `use thiserror::Error as
                // X;` — direct leaf match. `use te::Error;` after a
                // sibling `use thiserror as te;` — same shape after
                // first-segment expansion through `crate_aliases`.
                let matches_direct = path_matches_thiserror(&rule.thiserror_paths, &path);
                let matches_via_alias = !matches_direct
                    && match path.split_first() {
                        Some((first, rest)) => rule
                            .crate_aliases
                            .get(first)
                            .map(|&expanded| {
                                let mut expanded_path = Vec::with_capacity(path.len());
                                expanded_path.push(expanded);
                                expanded_path.extend_from_slice(rest);
                                path_matches_thiserror(&rule.thiserror_paths, &expanded_path)
                            })
                            .unwrap_or(false),
                        None => false,
                    };
                if matches_direct || matches_via_alias {
                    let local = rename
                        .map(|ident| ident.name)
                        .or_else(|| path.last().copied());
                    if let Some(local) = local {
                        rule.aliases.insert(local);
                    }
                }
            }
        },
        UseTreeKind::Glob(_) => {
            if matches!(phase, CollectPhase::CrateAliases) {
                // Globs don't introduce crate aliases.
                return;
            }
            // `use a::b::*;` brings the *direct* children of `a::b`
            // into scope, so the glob exposes a recognised leaf
            // only when the recognised path is exactly one segment
            // deeper than the glob's path. For `[[thiserror, Error]]`
            // plus `use thiserror::*;` the check is `2 == 1 + 1` and
            // `Error` is inserted; a glob that stops more than one
            // segment short of the leaf (e.g. a one-segment glob
            // against a three-segment path) fails the `+ 1` check, so
            // the rule does not falsely claim the leaf is in scope.
            //
            // The glob's path is also expanded through `crate_aliases`
            // so `use te::*;` after `use thiserror as te;` behaves
            // like `use thiserror::*;`.
            let expanded_path = match path.split_first() {
                Some((first, rest)) => match rule.crate_aliases.get(first) {
                    Some(&expanded_first) => {
                        let mut out = Vec::with_capacity(path.len());
                        out.push(expanded_first);
                        out.extend_from_slice(rest);
                        out
                    }
                    None => path.clone(),
                },
                None => path.clone(),
            };
            for cfg in &rule.thiserror_paths {
                if cfg.len() == expanded_path.len() + 1
                    && cfg.starts_with(&expanded_path)
                    && let Some(&last) = cfg.last()
                {
                    rule.aliases.insert(last);
                }
            }
        }
        UseTreeKind::Nested { items, .. } => {
            for (nested, _) in items {
                walk_use_tree_for_aliases(rule, nested, &path, phase);
            }
        }
    }
}
