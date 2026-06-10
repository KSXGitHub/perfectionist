//! Extract the `Config` struct and its accompanying `CONFIG_KEY`
//! constant out of a parsed rule source file. The result is the
//! [`ConfigDoc`] the renderer ultimately turns into the per-rule
//! Configuration section.

use crate::extract::serde_attrs::{
    apply_rename_all, doc_attrs_to_markdown, serde_has_default, serde_str_attr,
};
use crate::extract::shared::SharedTypes;
use crate::extract::ty::{collect_referenced_idents, find_type_doc, is_option, toml_type_label};
use crate::model::{ConfigDoc, ConfigField, Optionality};
use std::collections::BTreeSet;
use std::path::Path;
use syn::{Expr, ExprLit, Item, Lit};

/// Locate the rule's `Config` struct and its `CONFIG_KEY` constant
/// and bundle them — along with any project-local types the fields
/// reference — into a [`ConfigDoc`].
///
/// Every rule must declare both halves; a rule with no configurable
/// knobs uses an empty `Config {}`. Panics, naming the file, when a
/// rule declares neither half or only one of them.
pub(crate) fn extract_config(
    source_path: &Path,
    file: &syn::File,
    shared: &SharedTypes,
) -> ConfigDoc {
    let key = file.items.iter().find_map(|item| match item {
        Item::Const(item_const) if item_const.ident == "CONFIG_KEY" => match &*item_const.expr {
            Expr::Lit(ExprLit {
                lit: Lit::Str(literal),
                ..
            }) => Some(literal.value()),
            _ => None,
        },
        _ => None,
    });
    let config_struct = file.items.iter().find_map(|item| match item {
        Item::Struct(item_struct) if item_struct.ident == "Config" => Some(item_struct),
        _ => None,
    });
    let (key, config_struct) = match (key, config_struct) {
        (Some(key), Some(config_struct)) => (key, config_struct),
        (None, None) => panic!(
            "{}: a rule must declare both a `CONFIG_KEY` constant and a \
             `Config` struct. A rule with no configurable knobs uses an \
             empty `Config {{}}`.",
            source_path.display(),
        ),
        (Some(_), None) => panic!(
            "{}: declares `CONFIG_KEY` but no `Config` struct. Every rule \
             must declare both; add the `Config` struct (an empty \
             `Config {{}}` if the rule has no knobs).",
            source_path.display(),
        ),
        (None, Some(_)) => panic!(
            "{}: declares a `Config` struct but no `CONFIG_KEY` const. \
             Every rule must declare both; add the `CONFIG_KEY` constant.",
            source_path.display(),
        ),
    };
    let rename_all = serde_str_attr(&config_struct.attrs, "rename_all");
    // A container `#[serde(default)]` fills every missing field from a
    // default, so all the struct's fields are optional. Without it, a
    // field that is neither `Option<...>` nor individually defaulted is
    // required — the syntactic signal for a "Mandatory configuration on
    // opt-in rules" direction field such as `import_grouping`'s `style`.
    let container_default = serde_has_default(&config_struct.attrs);

    let named_fields = match &config_struct.fields {
        syn::Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let fields = named_fields
        .iter()
        .map(|field| {
            let rust_name = field
                .ident
                .as_ref()
                .expect("named field always has an ident")
                .to_string();
            // Precedence mirrors serde itself: per-field
            // `#[serde(rename = "...")]` wins, then the struct-
            // level `rename_all` style, then the raw Rust ident.
            let name = serde_str_attr(&field.attrs, "rename").unwrap_or_else(|| {
                rename_all
                    .as_deref()
                    .map(|style| apply_rename_all(style, &rust_name))
                    .unwrap_or(rust_name)
            });
            let optionality = if !container_default
                && !serde_has_default(&field.attrs)
                && !is_option(&field.ty)
            {
                Optionality::Mandatory
            } else {
                Optionality::Optional
            };
            ConfigField {
                name,
                type_label: toml_type_label(&field.ty, shared),
                optionality,
                doc_markdown: doc_attrs_to_markdown(&field.attrs),
            }
        })
        .collect();

    let mut referenced = Vec::new();
    let mut seen = BTreeSet::new();
    for field in &named_fields {
        collect_referenced_idents(&field.ty, &mut referenced, &mut seen, shared);
    }
    let custom_types = referenced
        .into_iter()
        .filter_map(|ident| find_type_doc(file, &ident, shared))
        .collect();

    ConfigDoc {
        key,
        fields,
        custom_types,
    }
}

#[cfg(test)]
mod tests;
