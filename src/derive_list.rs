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
//! re-parsed module AST. [`derive_entries`] is the underlying read of
//! one `derive(...)` list, and hands back each entry's span alongside
//! its name for a caller that rewrites the list rather than only asking
//! which traits are derived.

use crate::attr_tokens::attribute_calls_of;
use rustc_ast::tokenstream::TokenStream;
use rustc_ast::{Attribute, MetaItemKind};
use rustc_span::{Span, Symbol, sym};
use std::collections::HashSet;

/// One entry of a `derive(...)` list.
pub(crate) struct DeriveEntry {
    /// Final segment of the derived trait's path.
    pub(crate) name: Symbol,
    /// Source span of the entry, covering its full path text.
    pub(crate) span: Span,
}

/// Final path segment of every derive on the node, including
/// `#[cfg_attr(<cfg>, derive(...))]`-gated ones.
pub(crate) fn derive_names(attrs: &[Attribute]) -> HashSet<Symbol> {
    let mut names = HashSet::new();
    for call in attribute_calls_of(attrs) {
        if call.name == sym::derive {
            names.extend(
                derive_entries(call.tokens)
                    .into_iter()
                    .flatten()
                    .map(|entry| entry.name),
            );
        }
    }
    names
}

/// Every entry of a `derive(...)` list, given the `derive`'s argument
/// tokens. `None` when an entry is not a path: `derive` accepts paths
/// only, so such a list is malformed, and neither naming nor rewriting
/// an entry that could not be read is meaningful.
pub(crate) fn derive_entries(tokens: &TokenStream) -> Option<Vec<DeriveEntry>> {
    MetaItemKind::list_from_tokens(tokens.clone())?
        .iter()
        .map(|entry| {
            let segment = entry.meta_item()?.path.segments.last()?;
            Some(DeriveEntry {
                name: segment.ident.name,
                span: entry.span(),
            })
        })
        .collect()
}
