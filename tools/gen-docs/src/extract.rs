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
use std::path::{Path, PathBuf};

use pipe_trait::Pipe;
use proc_macro2::TokenStream;
use syn::{
    Attribute, Ident, Item, LitStr, Token,
    parse::{Parse, ParseStream},
};

use crate::extract::config::extract_config;
use crate::extract::serde_attrs::doc_attrs_to_markdown;
use crate::model::{DefaultState, Level, NAMESPACE, Rule};

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
    // `declare_tool_lint!` always lives in the parent file by
    // convention. Collect the macro bodies into owned `TokenStream`s
    // here so the merged-file step below (which consumes `file`) can't
    // invalidate the references — and so an accidental
    // `declare_tool_lint!` in a submodule wouldn't silently double-
    // register the rule. `TokenStream` clones are cheap; the bodies
    // are reparsed downstream anyway.
    let macro_items: Vec<TokenStream> = file
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
                item_macro.mac.tokens.clone().pipe(Some)
            }
            _ => None,
        })
        .collect();
    // Rules that have outgrown a flat `.rs` keep `CONFIG_KEY` and the
    // `Config` struct in a sibling `<rule>/config.rs` (see CLAUDE.md's
    // "When a rule grows past one screenful" section). The extractor
    // used to only see the parent file's items, so those rules'
    // configurations rendered as "Configuration: none." in the docs.
    // Merging submodule items into a single `syn::File` lets the
    // existing item walks in `extract_config` and `find_type_doc`
    // discover them without further plumbing.
    let merged_file = merge_submodule_items(source_path, file);
    if macro_items.is_empty() {
        // Silent skip for legitimate helper files. But if the
        // file *looks* rule-shaped — has a `CONFIG_KEY` const
        // or a `Config` struct — the missing macro is almost
        // certainly a typo (`declare_tool_lin!`, etc.) that
        // would otherwise drop the rule from the docs without
        // any signal.
        let looks_rule_shaped = merged_file.items.iter().any(|item| match item {
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

    // The convention is one rule per file with its own `Config`,
    // so `macro_items` is expected to have exactly one entry. The
    // code below still handles a multi-lint file defensively —
    // every emitted rule receives the same `config` value — but
    // no rule in the catalogue uses that shape today.
    let config = extract_config(source_path, &merged_file);
    let default_state = extract_default_state(source_path, &merged_file);
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
                syn::parse2::<DeclareToolLint>(tokens.clone()).unwrap_or_else(|error| {
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
            // Catalogue policy (see `README.md` and the
            // `Rule::default_state` doc comment): every rule's
            // `declare_tool_lint!` declares level `Warn`. The
            // default-disabled axis lives in
            // `pub(crate) const ENABLED_BY_DEFAULT: bool = false;`
            // alongside the macro, not in the level. A non-`Warn`
            // here means the source has drifted from the policy; we
            // panic so a doc regeneration catches it rather than
            // silently rendering the old "Default level" surface
            // we removed.
            if !matches!(level, Level::Warn) {
                panic!(
                    "{} declares level `{level_ident}`; catalogue policy is that \
                     every `declare_tool_lint!` uses level `Warn`. Move the \
                     off-by-default signal into \
                     `pub(crate) const ENABLED_BY_DEFAULT: bool = false;` \
                     next to the macro and change the level to `Warn`.",
                    source_path.display(),
                );
            }
            Rule {
                namespaced,
                default_state,
                short_desc: declaration.desc.value(),
                doc_markdown,
                relative_source: relative_source.clone(),
                config: config.clone(),
            }
        })
        .collect()
}

/// Find a `pub(crate) const ENABLED_BY_DEFAULT: bool = <literal>;`
/// item in `merged_file` and translate its boolean literal into a
/// [`DefaultState`]. The merged file already includes submodule
/// items, so the constant can live in either the flat rule file or
/// a sibling `<rule>/<concern>.rs` (in practice every rule keeps it
/// next to `declare_tool_lint!`). Absence of the constant means the
/// rule is on out of the box — the common case across the
/// catalogue — so the function returns [`DefaultState::Enabled`].
fn extract_default_state(source_path: &Path, merged_file: &syn::File) -> DefaultState {
    use syn::{Expr, ExprLit, Lit, Type, TypePath};

    for item in &merged_file.items {
        let Item::Const(item_const) = item else {
            continue;
        };
        if item_const.ident != "ENABLED_BY_DEFAULT" {
            continue;
        }
        let is_bool_type = matches!(
            &*item_const.ty,
            Type::Path(TypePath { path, qself: None }) if path.is_ident("bool"),
        );
        if !is_bool_type {
            panic!(
                "{}: `ENABLED_BY_DEFAULT` must be typed `bool`",
                source_path.display(),
            );
        }
        let Expr::Lit(ExprLit {
            lit: Lit::Bool(lit_bool),
            ..
        }) = &*item_const.expr
        else {
            panic!(
                "{}: `ENABLED_BY_DEFAULT` initializer must be a bool literal \
                 (`true` or `false`); other expressions defeat the gen-docs \
                 syntactic extraction",
                source_path.display(),
            );
        };
        return if lit_bool.value {
            DefaultState::Enabled
        } else {
            DefaultState::Disabled
        };
    }
    DefaultState::Enabled
}

/// Return a `syn::File` containing `parent`'s items followed by every
/// item declared in any `.rs` file of the sibling `<rule>/` directory,
/// when one exists. This collapses the directory-module rule layout
/// (parent `<rule>.rs` + `<rule>/<concern>.rs` siblings, see CLAUDE.md)
/// into a single flat item list so the downstream `Config` /
/// `CONFIG_KEY` lookup doesn't have to know which file declared what.
///
/// The walk is one level deep on purpose: every directory-module rule
/// in this repo keeps `config.rs` directly under `<rule>/`, never
/// nested further. Recursing would also pick up unrelated items from
/// helper sub-trees if any rule ever grew them.
///
/// File reads or `syn::parse_file` failures on submodule files emit a
/// warning and skip the file rather than panicking. The parent file
/// is the rule's source of truth; a malformed helper shouldn't take
/// the whole catalogue offline. Submodule entries are sorted by file
/// name so the merged item order is deterministic across platforms.
fn merge_submodule_items(source_path: &Path, parent: syn::File) -> syn::File {
    let Some(stem) = source_path.file_stem() else {
        return parent;
    };
    let Some(parent_dir) = source_path.parent() else {
        return parent;
    };
    let submodule_dir = parent_dir.join(stem);
    if !submodule_dir.is_dir() {
        return parent;
    }
    let entries = match fs::read_dir(&submodule_dir) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!(
                "warning: failed to read {}: {error}",
                submodule_dir.display(),
            );
            return parent;
        }
    };
    let mut submodule_paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    submodule_paths.sort();

    let mut merged = parent;
    for path in submodule_paths {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("warning: failed to read {}: {error}", path.display());
                continue;
            }
        };
        let submodule = match syn::parse_file(&source) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("warning: failed to parse {}: {error}", path.display());
                continue;
            }
        };
        merged.items.extend(submodule.items);
    }
    merged
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Allocate a fresh temp directory unique across both processes
    /// (cargo's test harness forks per binary) and across tests in
    /// the same binary (the atomic counter handles concurrent runs
    /// and any label collision). Mirrors the helper in
    /// `check_md.rs`'s test module; kept local so the two test
    /// modules stay self-contained.
    fn tempdir(label: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "perfectionist-gen-docs-extract-{label}-{}-{seq}",
            std::process::id(),
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    /// Directory-module rules keep `CONFIG_KEY` and `Config` in
    /// `<rule>/config.rs`. Merging the submodule items into the
    /// parent file is what lets `extract_config` find them; this
    /// pins the behaviour so the bug that sent
    /// `unicode_ellipsis_in_panic_messages` to "Configuration: none."
    /// can't silently come back.
    #[test]
    fn collect_rules_finds_config_in_submodule_file() {
        let base = tempdir("merge-submodule");
        let parent_path = base.join("demo_rule.rs");
        fs::write(
            &parent_path,
            r#"
                use rustc_session::declare_tool_lint;

                mod config;

                declare_tool_lint! {
                    /// ### What it does
                    /// Demo.
                    pub perfectionist::DEMO_RULE,
                    Warn,
                    "demo description"
                }
            "#,
        )
        .unwrap();
        let submodule_dir = base.join("demo_rule");
        fs::create_dir_all(&submodule_dir).unwrap();
        fs::write(
            submodule_dir.join("config.rs"),
            r#"
                const CONFIG_KEY: &str = "perfectionist::demo_rule";

                #[derive(serde::Deserialize)]
                #[serde(default, rename_all = "snake_case")]
                struct Config {
                    /// Demo knob.
                    knob: bool,
                }
            "#,
        )
        .unwrap();

        let rules = collect_rules(&base);
        assert_eq!(rules.len(), 1, "exactly one rule should be discovered");
        let config = rules[0]
            .config
            .as_ref()
            .expect("config in submodule should be merged into the rule's ConfigDoc");
        assert_eq!(config.key, "perfectionist::demo_rule");
        let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["knob"]);

        let _ = fs::remove_dir_all(&base);
    }
}
