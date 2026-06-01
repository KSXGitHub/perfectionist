//! Renders the canonical `use`-tree text for a group of leaves under a
//! chosen [`Style`]. Each function returns the text that goes between
//! `use ` and the trailing `;` for one statement; the caller prepends
//! the shared visibility / attributes and joins the statements.

use std::collections::BTreeMap;

use super::config::Style;
use super::model::{Leaf, LeafItem};

pub(super) fn render(style: Style, leaves: &[Leaf]) -> Vec<String> {
    match style {
        Style::Crate => render_crate(leaves),
        Style::Module => render_module(leaves),
        Style::Item => render_item(leaves),
    }
}

fn join(segments: &[String]) -> String {
    segments.join("::")
}

fn rename_suffix(rename: &Option<String>) -> String {
    match rename {
        Some(name) => format!(" as {name}"),
        None => String::new(),
    }
}

/// Order brace entries: `self` first, then names (case-insensitively),
/// then the glob `*` last. The full entry text is the final tiebreaker,
/// so entries that collide on the primary key (`self` vs `self as x`, or
/// `Foo` vs `foo`) still have a total, deterministic order rather than
/// relying on input order. Deduplicates exact repeats.
fn sort_entries(entries: &mut Vec<String>) {
    entries.sort_by(|left, right| entry_key(left).cmp(&entry_key(right)));
    entries.dedup();
}

fn entry_key(entry: &str) -> (u8, String, &str) {
    let group = if entry == "self" || entry.starts_with("self ") {
        0
    } else if entry == "*" {
        2
    } else {
        1
    };
    (group, entry.to_ascii_lowercase(), entry)
}

/// Wrap a module path's entries: a single non-`self` entry needs no
/// braces (`std::io::Read`, `std::io::*`); everything else does.
fn wrap(prefix: &str, entries: &[String]) -> String {
    if entries.len() == 1 && !needs_brace(&entries[0]) {
        format!("{prefix}::{}", entries[0])
    } else {
        format!("{prefix}::{{{}}}", entries.join(", "))
    }
}

fn needs_brace(entry: &str) -> bool {
    entry == "self" || entry.starts_with("self ")
}

/// `use serde;` / `use serde as alias;` — the standalone form every
/// style keeps as-is.
fn crate_item_body(leaf: &Leaf) -> String {
    let LeafItem::Named(name) = &leaf.item else {
        unreachable!("crate item is always a named leaf")
    };
    format!("{name}{}", rename_suffix(&leaf.rename))
}

fn item_entry(leaf: &Leaf) -> String {
    match &leaf.item {
        LeafItem::Named(name) => format!("{name}{}", rename_suffix(&leaf.rename)),
        LeafItem::Glob => "*".to_owned(),
        LeafItem::SelfMod => format!("self{}", rename_suffix(&leaf.rename)),
    }
}

fn render_item(leaves: &[Leaf]) -> Vec<String> {
    let mut out: Vec<String> = leaves
        .iter()
        .map(|leaf| {
            if leaf.is_crate_item() {
                crate_item_body(leaf)
            } else {
                let entry = item_entry(leaf);
                wrap(&join(&leaf.module), std::slice::from_ref(&entry))
            }
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

fn render_module(leaves: &[Leaf]) -> Vec<String> {
    let mut groups: BTreeMap<&[String], Vec<&Leaf>> = BTreeMap::new();
    for leaf in leaves {
        groups.entry(leaf.module.as_slice()).or_default().push(leaf);
    }
    let mut out = Vec::new();
    for (module, members) in groups {
        if module.is_empty() {
            for leaf in members {
                out.push(crate_item_body(leaf));
            }
            continue;
        }
        let mut entries: Vec<String> = members.iter().map(|leaf| item_entry(leaf)).collect();
        sort_entries(&mut entries);
        out.push(wrap(&join(module), &entries));
    }
    out.sort();
    out.dedup();
    out
}

#[derive(Default)]
struct Node {
    items: Vec<Leaf>,
    children: BTreeMap<String, Node>,
}

fn insert(node: &mut Node, module: &[String], leaf: &Leaf) {
    match module.split_first() {
        None => node.items.push(leaf.clone()),
        Some((head, rest)) => insert(node.children.entry(head.clone()).or_default(), rest, leaf),
    }
}

fn node_entries(node: &Node) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    // Every named item stands on its own — even when its name matches a
    // child module. `use a::b;` binds `b` in *every* namespace it exists
    // in (type, value, macro), so it must NOT be folded into the child
    // module's brace as `self`: `use a::b::{self};` re-imports only the
    // module, silently dropping any value or macro re-exported under the
    // same name and breaking the code that used it. The bare item and
    // the module's own items therefore sit side by side as
    // `a::{b, b::C}`, which is the most-collapsed form that preserves
    // every binding. An explicit `self` written in the source
    // (`LeafItem::SelfMod`) *is* a genuine module-only import and still
    // renders inside the child brace. See
    // <https://github.com/KSXGitHub/perfectionist/issues/186>.
    for item in &node.items {
        entries.push(item_entry(item));
    }
    for (segment, child) in &node.children {
        let sub = node_entries(child);
        entries.push(wrap(segment, &sub));
    }
    sort_entries(&mut entries);
    entries
}

fn render_crate(leaves: &[Leaf]) -> Vec<String> {
    // Everything goes into one trie keyed by path segment; a bare
    // `use foo;` lands as a named item at the root and renders as `foo`,
    // while `use foo::Bar;` descends into the `foo` child — the two fold
    // together via [`node_entries`]. Each top-level entry is one
    // statement.
    let mut root = Node::default();
    for leaf in leaves {
        insert(&mut root, &leaf.module, leaf);
    }
    let mut out = node_entries(&root);
    out.sort();
    out.dedup();
    out
}
