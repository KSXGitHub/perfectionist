//! Walk `src/rules/`, parse each `declare_tool_lint!` invocation,
//! and assemble the [`Rule`] values the renderer consumes. The
//! macro grammar is fixed by `rustc_session::declare_tool_lint!`
//! and the project's convention of placing the doc comment inside
//! the macro braces, so a hand-rolled `syn::parse::Parse` impl is
//! enough — we don't need to invoke the dylint driver or stand up a
//! rustc plugin host just to read these few fields.

pub(crate) mod config;
pub(crate) mod serde_attrs;
pub(crate) mod ty;

use std::fs;
use std::path::Path;

use proc_macro2::TokenStream;
use syn::{
    Attribute, Ident, Item, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::extract::config::extract_config;
use crate::extract::serde_attrs::doc_attrs_to_markdown;
use crate::model::{Level, NAMESPACE, Rule};

pub(crate) fn collect_rules(rules_dir: &Path) -> Vec<Rule> {
    let entries = fs::read_dir(rules_dir).expect("failed to read src/rules/");
    let mut rules = Vec::new();
    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        rules.extend(extract_rules(&path));
    }
    rules
}

fn extract_rules(source_path: &Path) -> Vec<Rule> {
    let source = fs::read_to_string(source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let file = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source_path.display()));
    let macro_items: Vec<&TokenStream> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Macro(item_macro)
                if item_macro
                    .mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "declare_tool_lint") =>
            {
                Some(&item_macro.mac.tokens)
            }
            _ => None,
        })
        .collect();
    if macro_items.is_empty() {
        // Silent skip for legitimate helper files. But if the
        // file *looks* rule-shaped — has a `CONFIG_KEY` const
        // or a `Config` struct — the missing macro is almost
        // certainly a typo (`declare_tool_lin!`, etc.) that
        // would otherwise drop the rule from the docs without
        // any signal.
        let looks_rule_shaped = file.items.iter().any(|item| match item {
            Item::Const(item_const) => item_const.ident == "CONFIG_KEY",
            Item::Struct(item_struct) => item_struct.ident == "Config",
            _ => false,
        });
        if looks_rule_shaped {
            eprintln!(
                "warning: {} looks rule-shaped (has `CONFIG_KEY` or `Config`) \
                 but has no `declare_tool_lint!` macro; the rule will not \
                 appear in the docs",
                source_path.display(),
            );
        }
        return Vec::new();
    }

    // A module with multiple `declare_tool_lint!` blocks is a
    // sub-lint family (e.g. `single_letter_names`'s four lints).
    // The configuration block is shared across the family — it
    // lives once at module scope and is keyed by the module name,
    // not by any individual sub-lint name — so attach it to every
    // rule emitted from this file.
    let config = extract_config(source_path, &file);
    let relative_source: std::path::PathBuf = source_path
        .components()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    macro_items
        .iter()
        .map(|tokens| {
            let declaration =
                syn::parse2::<DeclareToolLint>((*tokens).clone()).unwrap_or_else(|error| {
                    panic!(
                        "failed to parse declare_tool_lint! body in {}: {error}",
                        source_path.display()
                    )
                });
            let namespaced = format!(
                "{NAMESPACE}{}",
                declaration.name.to_string().to_ascii_lowercase()
            );
            let doc_markdown = doc_attrs_to_markdown(&declaration.attrs);
            let level_ident = declaration.level.to_string();
            let level: Level = level_ident.parse().unwrap_or_else(|_| {
                panic!(
                    "unknown lint level `{level_ident}` in {}",
                    source_path.display()
                )
            });
            Rule {
                namespaced,
                level,
                short_desc: declaration.desc.value(),
                doc_markdown,
                relative_source: relative_source.clone(),
                config: config.clone(),
            }
        })
        .collect()
}

/// Minimal grammar of `declare_tool_lint!`'s body. The macro itself
/// allows arbitrary `key: value` pairs after the description; we
/// don't currently surface them on the page, so the trailing tokens
/// are accepted and discarded.
struct DeclareToolLint {
    attrs: Vec<Attribute>,
    name: Ident,
    level: Ident,
    desc: LitStr,
}

impl Parse for DeclareToolLint {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let _vis: Token![pub] = input.parse()?;
        let _tool: Ident = input.parse()?;
        let _colon: Token![::] = input.parse()?;
        let name: Ident = input.parse()?;
        let _comma1: Token![,] = input.parse()?;
        let level: Ident = input.parse()?;
        let _comma2: Token![,] = input.parse()?;
        let desc: LitStr = input.parse()?;
        let _rest: TokenStream = input.parse()?;
        Ok(Self {
            attrs,
            name,
            level,
            desc,
        })
    }
}
