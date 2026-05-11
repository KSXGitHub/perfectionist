//! Extract the `Config` struct and its accompanying `CONFIG_KEY`
//! constant out of a parsed rule source file. The result is the
//! `ConfigDoc` the renderer ultimately turns into the per-rule
//! Configuration section.

use std::collections::BTreeSet;
use std::path::Path;

use syn::{Expr, ExprLit, Item, Lit};

use crate::extract::serde_attrs::{apply_rename_all, doc_attrs_to_markdown, serde_str_attr};
use crate::extract::ty::{collect_referenced_idents, find_type_doc, toml_type_label};
use crate::model::{ConfigDoc, ConfigField};

/// Locate the rule's `Config` struct and its `CONFIG_KEY` constant
/// and bundle them — along with any project-local types the fields
/// reference — into a `ConfigDoc`. Returns `None` when *both* the
/// constant and the struct are missing (a rule with no configuration
/// concept at all). Defining only one of the two prints a warning
/// and still returns `None`: the convention in this repo is that
/// every rule has both, and a half-defined Config is almost always
/// the author having dropped the other half.
pub(crate) fn extract_config(source_path: &Path, file: &syn::File) -> Option<ConfigDoc> {
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
        (None, None) => return None,
        (Some(_), None) => {
            eprintln!(
                "warning: {} declares CONFIG_KEY but no `Config` struct; \
                 skipping its configuration section",
                source_path.display(),
            );
            return None;
        }
        (None, Some(_)) => {
            eprintln!(
                "warning: {} declares a `Config` struct but no CONFIG_KEY const; \
                 skipping its configuration section",
                source_path.display(),
            );
            return None;
        }
    };
    let rename_all = serde_str_attr(&config_struct.attrs, "rename_all");

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
            ConfigField {
                name,
                type_label: toml_type_label(&field.ty),
                doc_markdown: doc_attrs_to_markdown(&field.attrs),
            }
        })
        .collect();

    let mut referenced = Vec::new();
    let mut seen = BTreeSet::new();
    for field in &named_fields {
        collect_referenced_idents(&field.ty, &mut referenced, &mut seen);
    }
    let custom_types = referenced
        .into_iter()
        .filter_map(|ident| find_type_doc(file, &ident))
        .collect();

    Some(ConfigDoc {
        key,
        fields,
        custom_types,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_field_honours_serde_rename() {
        // Walk a synthetic rule file through `extract_config` to
        // confirm that `#[serde(rename = "...")]` on a Config
        // field surfaces as the TOML key instead of the Rust ident.
        let source = r#"
            const CONFIG_KEY: &str = "perfectionist::demo";

            #[derive(serde::Deserialize)]
            #[serde(default, rename_all = "snake_case")]
            struct Config {
                #[serde(rename = "renamed-key")]
                rust_name: bool,
                plain_name: usize,
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let config = extract_config(Path::new("synthetic.rs"), &file)
            .expect("demo file declares CONFIG_KEY and Config");
        let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["renamed-key", "plain_name"]);
    }

    #[test]
    fn config_field_honours_struct_rename_all() {
        // `rename_all = "kebab-case"` on the Config struct should
        // surface every Rust field's snake-case identifier with
        // dashes. A per-field `#[serde(rename)]` still wins.
        let source = r#"
            const CONFIG_KEY: &str = "perfectionist::demo";

            #[derive(serde::Deserialize)]
            #[serde(default, rename_all = "kebab-case")]
            struct Config {
                also_flag: bool,
                some_other_key: usize,
                #[serde(rename = "explicit")]
                overridden: bool,
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let config = extract_config(Path::new("synthetic.rs"), &file)
            .expect("demo file declares CONFIG_KEY and Config");
        let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["also-flag", "some-other-key", "explicit"]);
    }
}
