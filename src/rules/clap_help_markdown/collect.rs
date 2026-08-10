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
use std::collections::{BTreeSet, HashMap, HashSet};

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

/// Every clap-bound, doc-commented, non-overridden node in the crate,
/// indexed two ways for the rule's two checks.
#[derive(Default)]
pub(super) struct DocNodes {
    /// Byte offset of *each* `///` line of a node → its [`NodeState`].
    /// Keyed per line because an interleaved non-doc attribute splits a
    /// node's `///` run into several [`crate::comment_walk`] blocks, one
    /// per consecutive run, each keyed by its own first line's offset —
    /// so recording only the first would leave a later run's markdown
    /// unscanned. The markdown scan looks each block up here.
    pub(super) by_line: HashMap<u32, NodeState>,
    /// Byte offset of the *first* `///` line of each node — exactly one
    /// entry per node. The `require_help_override` check anchors its
    /// single per-node diagnostic here, so a node whose `///` run is
    /// split across several blocks is still flagged just once (at its
    /// first block).
    pub(super) first_lines: HashSet<u32>,
}

impl DocNodes {
    /// No clap-bound, doc-commented, non-overridden node exists — so
    /// neither check has anything to do. `by_line` being empty implies
    /// `first_lines` is too (a node contributes to both or to neither).
    pub(super) fn is_empty(&self) -> bool {
        self.by_line.is_empty()
    }
}

/// The final path segment of every derive macro that makes its
/// container a clap help source. Matched by name (the re-parse runs
/// before name resolution), which catches `clap::Parser`, an imported
/// `Parser`, and a same-name re-export (`pub use clap::Parser;`); a
/// derive renamed through `use clap::Parser as P;` is not caught — the
/// same accepted limitation the sibling `perfectionist::unordered_derives`
/// and `perfectionist::thiserror_usage` rules carry.
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
///
/// The override-key set is clap's own fixed set ([`super::config::OVERRIDE_KEYS`]),
/// interned once here rather than threaded from configuration — a project
/// cannot add to or remove from clap's help-text override vocabulary.
pub(super) fn collect_doc_nodes(
    crates: &[Crate],
    live_module_spans: &HashSet<SpanRange>,
) -> DocNodes {
    let override_keys: BTreeSet<Symbol> = super::config::OVERRIDE_KEYS
        .iter()
        .map(|key| Symbol::intern(key))
        .collect();
    let mut nodes = DocNodes::default();
    for krate in crates {
        walk_items(&krate.items, live_module_spans, &override_keys, &mut nodes);
    }
    nodes
}

fn walk_items(
    items: &[Box<Item>],
    live_module_spans: &HashSet<SpanRange>,
    override_keys: &BTreeSet<Symbol>,
    nodes: &mut DocNodes,
) {
    for item in items {
        match &item.kind {
            ItemKind::Struct(_, _, data) if has_clap_derive(&item.attrs) => {
                record_node(&item.attrs, override_keys, nodes);
                record_fields(data, override_keys, nodes);
            }
            ItemKind::Enum(_, _, def) if has_clap_derive(&item.attrs) => {
                // A `ValueEnum`'s own type-level doc never reaches
                // `--help`: clap takes the argument help from the
                // `Args`/`Parser` field whose type is the enum, and the
                // per-value help from the variant docs. So skip the
                // container doc for a value-only enum. The variant docs
                // below can still reach `--help`, so they stay in scope;
                // `record_node` then drops any that override their help.
                if enum_container_doc_reaches_help(&item.attrs) {
                    record_node(&item.attrs, override_keys, nodes);
                }
                for variant in &def.variants {
                    record_node(&variant.attrs, override_keys, nodes);
                    record_fields(&variant.data, override_keys, nodes);
                }
            }
            // Descend into inline `mod { ... }` bodies, but only those
            // live in the compiled crate. A re-parse keeps cfg-disabled
            // inline modules, which have no HIR node and so could not be
            // silenced by a local `#[allow]` — the guard the
            // `import_grouping_mismatch` reference implementation documents.
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, spans))
                if live_module_spans.contains(&(spans.inner_span.lo(), spans.inner_span.hi())) =>
            {
                walk_items(items, live_module_spans, override_keys, nodes);
            }
            _ => {}
        }
    }
}

fn record_fields(data: &VariantData, override_keys: &BTreeSet<Symbol>, nodes: &mut DocNodes) {
    for field in data.fields() {
        record_node(&field.attrs, override_keys, nodes);
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
fn record_node(attrs: &[Attribute], override_keys: &BTreeSet<Symbol>, nodes: &mut DocNodes) {
    let mut doc_los = attrs
        .iter()
        .filter(|attr| matches!(attr.kind, AttrKind::DocComment(..)))
        .map(|attr| attr.span.lo().0)
        .peekable();
    // Ascending because `attrs` is in source order, so the first is the
    // node's opening `///` line — the anchor `first_lines` records.
    let Some(&first_line) = doc_los.peek() else {
        return;
    };
    if has_override(attrs, override_keys) {
        return;
    }
    let state = if has_verbatim(attrs) {
        NodeState::Verbatim
    } else {
        NodeState::Normal
    };
    nodes.first_lines.insert(first_line);
    for doc_lo in doc_los {
        nodes.by_line.insert(doc_lo, state);
    }
}

fn has_override(attrs: &[Attribute], override_keys: &BTreeSet<Symbol>) -> bool {
    clap_namespace_items(attrs).into_iter().any(|inner| {
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
    clap_namespace_items(attrs).into_iter().any(|inner| {
        let Some(meta) = inner.meta_item() else {
            return false;
        };
        matches!(meta.kind, MetaItemKind::Word) && meta.has_name(verbatim)
    })
}

/// Collect the inner meta items of every `#[clap(...)]` / `#[arg(...)]`
/// / `#[command(...)]` / `#[value(...)]` attribute on the node, looking
/// through any `#[cfg_attr(<cfg>, clap(...))]` gate — the form a crate
/// uses to keep its clap attributes behind a `cli` feature. The derive
/// detector ([`has_clap_derive`]) already descends into `cfg_attr`;
/// override and `verbatim_doc_comment` detection must match, or a gated
/// `#[clap(help = "...")]` is missed and the doc comment is wrongly
/// flagged as leaking into `--help`.
fn clap_namespace_items(attrs: &[Attribute]) -> Vec<MetaItemInner> {
    let mut out = Vec::new();
    for attr in attrs {
        if is_clap_namespace(attr) {
            out.extend(attr.meta_item_list().unwrap_or_default());
        } else if attr.has_name(sym::cfg_attr)
            && let Some(list) = attr.meta_item_list()
        {
            collect_cfg_attr_clap_items(list.get(1..).unwrap_or_default(), &mut out);
        }
    }
    out
}

/// Pull the inner items of clap-namespace meta items out of the
/// conditionally-applied tail of a `#[cfg_attr(...)]`, recursing through
/// nested `cfg_attr` the same way [`cfg_attr_has_derive`] does.
fn collect_cfg_attr_clap_items(items: &[MetaItemInner], out: &mut Vec<MetaItemInner>) {
    for item in items {
        let Some(meta) = item.meta_item() else {
            continue;
        };
        if let MetaItemKind::List(inner) = &meta.kind {
            if is_clap_namespace_path(meta) {
                out.extend(inner.iter().cloned());
            } else if meta.has_name(sym::cfg_attr) {
                collect_cfg_attr_clap_items(inner.get(1..).unwrap_or_default(), out);
            }
        }
    }
}

fn is_clap_namespace(attr: &Attribute) -> bool {
    CLAP_ATTR_NAMESPACES
        .iter()
        .any(|ns| attr.has_name(Symbol::intern(ns)))
}

fn is_clap_namespace_path(meta: &rustc_ast::MetaItem) -> bool {
    meta.path.segments.last().is_some_and(|segment| {
        CLAP_ATTR_NAMESPACES
            .iter()
            .any(|ns| segment.ident.name == Symbol::intern(ns))
    })
}

/// The clap derives that turn a *container's* own (type-level) doc
/// comment into `--help` text: a `Parser` / `CommandFactory` (and the
/// legacy `Clap`) type's doc becomes the command's `about` / `long_about`.
/// Used to keep a `ValueEnum` container doc in scope when the enum *also*
/// derives one of these — an unusual but representable combination.
const COMMAND_ABOUT_DERIVES: &[&str] = &["Parser", "CommandFactory", "Clap"];

/// Whether the node's attributes derive a clap help-source trait,
/// including a `#[cfg_attr(<cfg>, derive(...))]`-gated derive.
fn has_clap_derive(attrs: &[Attribute]) -> bool {
    has_derive_matching(attrs, &|name| CLAP_DERIVES.contains(&name))
}

/// Whether an enum's own (type-level) doc comment can reach `--help`. A
/// `ValueEnum`'s does not — clap reads its argument help from the field
/// that *uses* the enum, and its per-value help from the variant docs, so
/// the enum-level doc is rustdoc-only. If the enum *also* derives a
/// command-producing trait (`Parser` / `CommandFactory`, or the legacy
/// `Clap`), which does consume the type doc, the container doc is live.
fn enum_container_doc_reaches_help(attrs: &[Attribute]) -> bool {
    !has_derive_matching(attrs, &|name| name == "ValueEnum")
        || has_derive_matching(attrs, &|name| COMMAND_ABOUT_DERIVES.contains(&name))
}

/// Whether any `#[derive(...)]` on the node — including a
/// `#[cfg_attr(<cfg>, derive(...))]`-gated one — names a derive matching
/// `pred`.
fn has_derive_matching(attrs: &[Attribute], pred: &dyn Fn(&str) -> bool) -> bool {
    attrs.iter().any(|attr| {
        if attr.has_name(sym::derive) {
            attr.meta_item_list()
                .is_some_and(|entries| derive_entries_match(&entries, pred))
        } else if attr.has_name(sym::cfg_attr) {
            attr.meta_item_list()
                .is_some_and(|args| cfg_attr_has_derive(args.get(1..).unwrap_or(&[]), pred))
        } else {
            false
        }
    })
}

fn cfg_attr_has_derive(items: &[MetaItemInner], pred: &dyn Fn(&str) -> bool) -> bool {
    items.iter().any(|item| {
        let Some(meta) = item.meta_item() else {
            return false;
        };
        if meta.has_name(sym::derive) {
            matches!(&meta.kind, MetaItemKind::List(entries) if derive_entries_match(entries, pred))
        } else if meta.has_name(sym::cfg_attr) {
            matches!(&meta.kind, MetaItemKind::List(args) if cfg_attr_has_derive(args.get(1..).unwrap_or(&[]), pred))
        } else {
            false
        }
    })
}

fn derive_entries_match(entries: &[MetaItemInner], pred: &dyn Fn(&str) -> bool) -> bool {
    entries.iter().any(|entry| {
        entry
            .meta_item()
            .and_then(|meta| meta.path.segments.last())
            .is_some_and(|segment| pred(segment.ident.name.as_str()))
    })
}
