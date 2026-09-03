//! Reading the trait names out of a `#[derive(...)]` list.
//!
//! [`derive_names`] collects the final path segment of every derived
//! trait on a node, looking through `#[cfg_attr(<cfg>, derive(...))]` —
//! a gated derive still governs what its helper attributes mean wherever
//! it applies. Matching by final segment catches `some_crate::Trait`, a
//! plain `Trait` imported from it, and a same-name re-export; a derive
//! renamed through `use some_crate::Trait as T;` is not caught.
//!
//! Read from tokens rather than parsed meta-items so it works on a
//! re-parsed module AST. Several rules read derive lists their own way
//! (`unordered_derives`, `clap_help_markdown`); this is the shared form
//! for the "which traits are derived" question, where entry spans are
//! not needed.

use crate::attr_tokens::attribute_calls_of;
use rustc_ast::tokenstream::TokenStream;
use rustc_ast::{Attribute, MetaItemInner, MetaItemKind};
use rustc_span::{Symbol, sym};
use std::collections::HashSet;

/// Final path segment of every derive on the node, including
/// `#[cfg_attr(<cfg>, derive(...))]`-gated ones.
pub(crate) fn derive_names(attrs: &[Attribute]) -> HashSet<Symbol> {
    let mut names = HashSet::new();
    for call in attribute_calls_of(attrs) {
        if call.name == sym::derive {
            names.extend(derive_entries(call.tokens));
        }
    }
    names
}

/// Final path segment of each entry in a `derive(...)` list.
fn derive_entries(tokens: &TokenStream) -> Vec<Symbol> {
    let Some(entries) = MetaItemKind::list_from_tokens(tokens.clone()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(MetaItemInner::meta_item)
        .filter_map(|meta| meta.path.segments.last())
        .map(|segment| segment.ident.name)
        .collect()
}
