//! Per-item detection. Given the alias maps the [`scan`](super::scan)
//! pass populated, decide whether a `use` / `extern crate` item, a
//! struct/enum's `#[derive(...)]` list, or its `#[error(...)]`
//! attributes reference `thiserror`, and emit the corresponding
//! diagnostic. Entry points (`check_struct`, `check_enum`,
//! `check_use`) are `pub(super)` and called from the rule's
//! `EarlyLintPass` driver.

use rustc_ast::{
    AttrVec, EnumDef, Item, MetaItemInner, MetaItemKind, UseTree, UseTreeKind, VariantData,
};
use rustc_lint::EarlyContext;
use rustc_span::{Symbol, kw, sym};

use super::config::{PreferDeriveMoreOverThiserror, path_matches_thiserror};
use super::emit::{
    emit_derive, emit_use, flag_enum_error_attrs, flag_error_attrs, flag_variant_data_error_attrs,
};

impl PreferDeriveMoreOverThiserror {
    pub(super) fn check_struct(&self, cx: &EarlyContext<'_>, attrs: &AttrVec, data: &VariantData) {
        if !self.check_derive_attrs(cx, attrs) {
            return;
        }
        flag_error_attrs(cx, attrs);
        flag_variant_data_error_attrs(cx, data);
    }

    pub(super) fn check_enum(&self, cx: &EarlyContext<'_>, attrs: &AttrVec, def: &EnumDef) {
        if !self.check_derive_attrs(cx, attrs) {
            return;
        }
        flag_error_attrs(cx, attrs);
        flag_enum_error_attrs(cx, def);
    }

    /// Emit on every `use` statement whose tree touches a configured
    /// thiserror crate at any depth. Handles the three common shapes
    /// in one pass:
    ///
    /// - `use thiserror::Error;` — outer prefix's first segment is
    ///   `thiserror`, matches immediately.
    /// - `use thiserror::{Error, ErrorKind};` — same as above; the
    ///   nested children inherit the outer prefix, so we don't have
    ///   to walk them.
    /// - `use {thiserror::Error, std::io};` — top-level braced form
    ///   with an empty outer prefix; recurse into the children and
    ///   flag if any child's prefix names a thiserror crate.
    pub(super) fn check_use(&self, cx: &EarlyContext<'_>, item: &Item, use_tree: &UseTree) {
        if self.use_tree_references_thiserror(use_tree) {
            emit_use(cx, item.span);
        }
    }

    fn use_tree_references_thiserror(&self, tree: &UseTree) -> bool {
        let first = tree
            .prefix
            .segments
            .iter()
            .map(|segment| segment.ident.name)
            .find(|name| *name != kw::PathRoot);
        if let Some(name) = first {
            // Direct match — `use thiserror::...`. Also match through
            // `crate_aliases` so that `use te::Error;` after a sibling
            // `use thiserror as te;` is flagged on its own line, not
            // just on the rename.
            return self.thiserror_crates.contains(&name) || self.crate_aliases.contains_key(&name);
        }
        // Empty outer prefix — recurse into nested children. (A
        // glob with an empty prefix is `use *;` and not valid Rust;
        // a simple with an empty prefix is also invalid. Only the
        // nested form needs handling here.)
        if let UseTreeKind::Nested { items, .. } = &tree.kind {
            return items
                .iter()
                .any(|(nested, _)| self.use_tree_references_thiserror(nested));
        }
        false
    }

    /// Walk a struct or enum's outer attributes and emit a
    /// diagnostic on every derive entry that resolves to a
    /// configured thiserror path. Recurses through `#[cfg_attr]`
    /// wrappers so that conditional derives are not silently
    /// missed. Returns `true` when at least one entry matched,
    /// signalling that the caller should also flag `#[error(...)]`
    /// attributes elsewhere on the item.
    fn check_derive_attrs(&self, cx: &EarlyContext<'_>, attrs: &AttrVec) -> bool {
        let mut thiserror_derived = false;
        for attr in attrs {
            if attr.has_name(sym::derive)
                && let Some(entries) = attr.meta_item_list()
                && self.check_derive_entries(cx, &entries)
            {
                thiserror_derived = true;
            } else if attr.has_name(sym::cfg_attr)
                && let Some(args) = attr.meta_item_list()
                && self.check_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]))
            {
                thiserror_derived = true;
            }
        }
        thiserror_derived
    }

    /// Inspect the attribute-list payload of a `#[cfg_attr(pred, ...)]`
    /// — the slice after the predicate. Each payload entry is itself
    /// an attribute shape; recognise nested `derive(...)` and
    /// further nested `cfg_attr(...)` and recurse.
    fn check_cfg_attr_payload(&self, cx: &EarlyContext<'_>, items: &[MetaItemInner]) -> bool {
        let mut found = false;
        for item in items {
            let Some(meta) = item.meta_item() else {
                continue;
            };
            if meta.has_name(sym::derive)
                && let MetaItemKind::List(entries) = &meta.kind
                && self.check_derive_entries(cx, entries)
            {
                found = true;
            } else if meta.has_name(sym::cfg_attr)
                && let MetaItemKind::List(args) = &meta.kind
                && self.check_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]))
            {
                found = true;
            }
        }
        found
    }

    fn check_derive_entries(&self, cx: &EarlyContext<'_>, entries: &[MetaItemInner]) -> bool {
        let mut thiserror_derived = false;
        for entry in entries {
            let Some(meta) = entry.meta_item() else {
                continue;
            };
            let segments: Vec<Symbol> = meta
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.name)
                .filter(|name| *name != kw::PathRoot)
                .collect();
            if self.is_thiserror_derive(&segments) {
                thiserror_derived = true;
                emit_derive(cx, entry.span());
            }
        }
        thiserror_derived
    }

    fn is_thiserror_derive(&self, segments: &[Symbol]) -> bool {
        if path_matches_thiserror(&self.thiserror_paths, segments) {
            return true;
        }
        // Single-segment derive entry (`#[derive(Error)]`) matches
        // when the crate has a sibling `use thiserror::Error;` (or
        // aliased / glob form) somewhere in scope.
        if let [name] = segments
            && self.aliases.contains(name)
        {
            return true;
        }
        // Crate-aliased derive entry (`#[derive(te::Error)]` paired
        // with `use thiserror as te;`): expand the first segment to
        // the original crate name and re-match against configured
        // paths.
        if let [first, rest @ ..] = segments
            && let Some(&expanded_first) = self.crate_aliases.get(first)
        {
            let mut expanded = Vec::with_capacity(segments.len());
            expanded.push(expanded_first);
            expanded.extend_from_slice(rest);
            return path_matches_thiserror(&self.thiserror_paths, &expanded);
        }
        false
    }
}
