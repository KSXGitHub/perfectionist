//! Extract the `Config` struct and its accompanying `CONFIG_KEY`
//! constant out of a parsed rule source file. The result is the
//! `ConfigDoc` the renderer ultimately turns into the per-rule
//! Configuration section.

use std::collections::BTreeSet;
use std::path::Path;

use syn::{Expr, ExprLit, Item, Lit};

use crate::extract::serde_attrs::{apply_rename_all, doc_attrs_to_markdown, serde_str_attr};
use crate::extract::shared::SharedTypes;
use crate::extract::ty::{collect_referenced_idents, find_type_doc, toml_type_label};
use crate::model::{ConfigDoc, ConfigField};

/// Locate the rule's `Config` struct and its `CONFIG_KEY` constant
/// and bundle them — along with any project-local types the fields
/// reference — into a `ConfigDoc`.
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
            let doc_markdown = doc_attrs_to_markdown(&field.attrs);
            ConfigField {
                name,
                type_label: toml_type_label(&field.ty, shared),
                required: is_mandatory(&doc_markdown),
                doc_markdown,
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

/// Whether a `Config` field is mandatory rather than optional.
///
/// There is no syntactic signal to key off: a mandatory direction
/// field (`self_import`'s `style`) is `Option<T>` with no default, which
/// looks identical to a genuinely-optional `Option<T>` field. The
/// convention is therefore that a mandatory field's doc comment leads
/// with a bold `**Mandatory…**` sentinel — ordinary emphasis in
/// rustdoc, recognised here to drive the `mandatory` badge. See the
/// "Mandatory configuration on opt-in rules" section of
/// `IMPLEMENTATION_CONVENTIONS.md`.
fn is_mandatory(doc_markdown: &str) -> bool {
    doc_markdown.trim_start().starts_with("**Mandatory")
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
        let config = extract_config(Path::new("synthetic.rs"), &file, &SharedTypes::default());
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
        let config = extract_config(Path::new("synthetic.rs"), &file, &SharedTypes::default());
        let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["also-flag", "some-other-key", "explicit"]);
    }

    #[test]
    #[should_panic(expected = "must declare both")]
    fn extract_config_rejects_a_rule_with_no_config() {
        // A rule file that declares neither `CONFIG_KEY` nor `Config`
        // is a convention violation: "no configuration" must be spelt
        // as an empty `Config {}`, never as an absent pair. The
        // extractor panics so a doc regeneration catches it instead of
        // silently dropping the configuration section.
        let source = "pub struct SomeRule;";
        let file = syn::parse_file(source).unwrap();
        let _ = extract_config(Path::new("no_config.rs"), &file, &SharedTypes::default());
    }

    #[test]
    #[should_panic(expected = "no `Config` struct")]
    fn extract_config_rejects_a_half_defined_config_key_only() {
        // Declaring `CONFIG_KEY` without a `Config` struct is the
        // usual shape of an author having dropped one half; reject it.
        let source = r#"const CONFIG_KEY: &str = "perfectionist::demo";"#;
        let file = syn::parse_file(source).unwrap();
        let _ = extract_config(Path::new("half.rs"), &file, &SharedTypes::default());
    }

    #[test]
    #[should_panic(expected = "no `CONFIG_KEY` const")]
    fn extract_config_rejects_a_half_defined_struct_only() {
        let source = "struct Config {}";
        let file = syn::parse_file(source).unwrap();
        let _ = extract_config(Path::new("half.rs"), &file, &SharedTypes::default());
    }
}
