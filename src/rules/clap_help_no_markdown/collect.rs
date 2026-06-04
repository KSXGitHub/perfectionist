//! Walk the re-parsed crate AST to find clap-derived containers and the
//! state of every doc-commented node within them.
//!
//! Detection runs on the freshly re-parsed module ASTs (see
//! [`crate::module_reparse`]) rather than the HIR, because a
//! `#[derive(...)]` attribute is consumed during macro expansion and is
//! gone by the late pass. Re-parsing keeps the written `#[derive(...)]`,
//! `#[arg(...)]`, and doc-comment attributes intact and reaches every
//! module file — including separate-file `mod foo;` submodules an
//! `EarlyLintPass` would miss.
//!
//! The result maps the source byte offset of each documented node's
//! first `///` line to that node's [`NodeState`]. The rule's late pass
//! walks doc-comment blocks with [`crate::comment_walk`] and looks each
//! block up by its start offset, so a block whose offset is absent
//! belongs to a non-clap item (or an overridden node) and is skipped.

use crate::module_reparse::SpanRange;
use rustc_ast::{
    AttrKind, Attribute, Crate, Item, ItemKind, MetaItemInner, MetaItemKind, ModKind, VariantData,
};
use rustc_span::{Symbol, sym};
use std::collections::{HashMap, HashSet};

/// What the lint should do with a clap-bound, doc-commented node.
#[derive(Debug, Clone, Copy)]
pub(super) enum NodeState {
    /// Flag forbidden markdown normally, with the code-span autofix.
    Normal,
    /// The node carries `#[clap(verbatim_doc_comment)]` (or the
    /// `command` / `arg` form): the doc comment is rendered verbatim
    /// into `--help`, so flag with a softer "markdown will leak" note
    /// and no autofix.
    Verbatim,
}

/// The final path segment of every derive macro that makes its
/// container a clap help source. Matched by name (the re-parse runs
/// before name resolution), which catches `clap::Parser`, an imported
/// `Parser`, and a same-name re-export (`pub use clap::Parser;`); a
/// derive renamed through `use clap::Parser as P;` is not caught — the
/// same accepted limitation the sibling `perfectionist::derive_ordering`
/// and `perfectionist::prefer_derive_more_over_thiserror` rules carry.
const CLAP_DERIVES: &[&str] = &[
    "Parser",
    "Args",
    "Subcommand",
    "ValueEnum",
    "CommandFactory",
    "Clap",
];

/// Attribute namespaces whose contents can carry a help override or the
/// `verbatim_doc_comment` flag.
const CLAP_ATTR_NAMESPACES: &[&str] = &["clap", "arg", "command", "value"];

/// Build the doc-node map for every clap-derived container in `crates`.
pub(super) fn collect_doc_nodes(
    crates: &[Crate],
    live_module_spans: &HashSet<SpanRange>,
    override_keys: &std::collections::BTreeSet<Symbol>,
) -> HashMap<u32, NodeState> {
    let mut map = HashMap::new();
    for krate in crates {
        walk_items(&krate.items, live_module_spans, override_keys, &mut map);
    }
    map
}

fn walk_items(
    items: &[Box<Item>],
    live_module_spans: &HashSet<SpanRange>,
    override_keys: &std::collections::BTreeSet<Symbol>,
    map: &mut HashMap<u32, NodeState>,
) {
    for item in items {
        match &item.kind {
            ItemKind::Struct(_, _, data) if has_clap_derive(&item.attrs) => {
                record_node(&item.attrs, override_keys, map);
                record_fields(data, override_keys, map);
            }
            ItemKind::Enum(_, _, def) if has_clap_derive(&item.attrs) => {
                record_node(&item.attrs, override_keys, map);
                for variant in &def.variants {
                    record_node(&variant.attrs, override_keys, map);
                    record_fields(&variant.data, override_keys, map);
                }
            }
            // Descend into inline `mod { ... }` bodies, but only those
            // live in the compiled crate. A re-parse keeps cfg-disabled
            // inline modules, which have no HIR node and so could not be
            // silenced by a local `#[allow]` — the guard the
            // `import_grouping` reference implementation documents.
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, spans))
                if live_module_spans.contains(&(spans.inner_span.lo(), spans.inner_span.hi())) =>
            {
                walk_items(items, live_module_spans, override_keys, map);
            }
            _ => {}
        }
    }
}

fn record_fields(
    data: &VariantData,
    override_keys: &std::collections::BTreeSet<Symbol>,
    map: &mut HashMap<u32, NodeState>,
) {
    for field in data.fields() {
        record_node(&field.attrs, override_keys, map);
    }
}

/// Record one documented node, keyed by the start offset of every `///`
/// line it carries. A node with no doc comment, or one whose help text
/// is overridden by an explicit attribute, contributes nothing — the
/// former has no comment block to flag, the latter is deliberately
/// silent.
///
/// Every doc-line offset is recorded, not just the first, because a
/// node's `///` lines split into more than one [`crate::comment_walk`]
/// block when a non-doc attribute is interleaved
/// (`/// a` `#[arg(long)]` `/// b`): the walker yields one block per
/// consecutive run, each keyed by its own first line's offset. Recording
/// only the minimum offset would leave the later run's markdown
/// unscanned even though clap concatenates both runs into `--help`. The
/// surplus mid-run offsets are harmless: the walker only ever looks up a
/// run's first line.
fn record_node(
    attrs: &[Attribute],
    override_keys: &std::collections::BTreeSet<Symbol>,
    map: &mut HashMap<u32, NodeState>,
) {
    let mut doc_los = attrs
        .iter()
        .filter(|attr| matches!(attr.kind, AttrKind::DocComment(..)))
        .map(|attr| attr.span.lo().0)
        .peekable();
    if doc_los.peek().is_none() {
        return;
    }
    if has_override(attrs, override_keys) {
        return;
    }
    let state = if has_verbatim(attrs) {
        NodeState::Verbatim
    } else {
        NodeState::Normal
    };
    for doc_lo in doc_los {
        map.insert(doc_lo, state);
    }
}

fn has_override(attrs: &[Attribute], override_keys: &std::collections::BTreeSet<Symbol>) -> bool {
    clap_namespace_items(attrs).any(|inner| {
        let Some(meta) = inner.meta_item() else {
            return false;
        };
        matches!(meta.kind, MetaItemKind::NameValue(_))
            && meta
                .path
                .segments
                .last()
                .is_some_and(|segment| override_keys.contains(&segment.ident.name))
    })
}

fn has_verbatim(attrs: &[Attribute]) -> bool {
    let verbatim = Symbol::intern("verbatim_doc_comment");
    clap_namespace_items(attrs).any(|inner| {
        let Some(meta) = inner.meta_item() else {
            return false;
        };
        matches!(meta.kind, MetaItemKind::Word) && meta.has_name(verbatim)
    })
}

/// Iterate the inner meta items of every `#[clap(...)]` / `#[arg(...)]`
/// / `#[command(...)]` / `#[value(...)]` attribute on the node.
fn clap_namespace_items(attrs: &[Attribute]) -> impl Iterator<Item = MetaItemInner> + '_ {
    attrs
        .iter()
        .filter(|attr| is_clap_namespace(attr))
        .flat_map(|attr| attr.meta_item_list().unwrap_or_default())
}

fn is_clap_namespace(attr: &Attribute) -> bool {
    CLAP_ATTR_NAMESPACES
        .iter()
        .any(|ns| attr.has_name(Symbol::intern(ns)))
}

/// Whether the node's attributes derive a clap help-source trait,
/// including a `#[cfg_attr(<cfg>, derive(...))]`-gated derive.
fn has_clap_derive(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.has_name(sym::derive) {
            attr.meta_item_list()
                .is_some_and(|entries| derive_entries_match(&entries))
        } else if attr.has_name(sym::cfg_attr) {
            attr.meta_item_list()
                .is_some_and(|args| cfg_attr_has_clap_derive(args.get(1..).unwrap_or(&[])))
        } else {
            false
        }
    })
}

fn cfg_attr_has_clap_derive(items: &[MetaItemInner]) -> bool {
    items.iter().any(|item| {
        let Some(meta) = item.meta_item() else {
            return false;
        };
        if meta.has_name(sym::derive) {
            matches!(&meta.kind, MetaItemKind::List(entries) if derive_entries_match(entries))
        } else if meta.has_name(sym::cfg_attr) {
            matches!(&meta.kind, MetaItemKind::List(args) if cfg_attr_has_clap_derive(args.get(1..).unwrap_or(&[])))
        } else {
            false
        }
    })
}

fn derive_entries_match(entries: &[MetaItemInner]) -> bool {
    entries.iter().any(|entry| {
        entry
            .meta_item()
            .and_then(|meta| meta.path.segments.last())
            .is_some_and(|segment| CLAP_DERIVES.contains(&segment.ident.name.as_str()))
    })
}
