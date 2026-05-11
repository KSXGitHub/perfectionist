//! Generate the static documentation site for perfectionist's
//! implemented lints.
//!
//! Reads each `src/rules/*.rs` (skipping `mod.rs`-style index files
//! that don't declare a lint), locates the `declare_tool_lint!`
//! invocation, and pulls four things out of it: the lint identifier
//! (`pub perfectionist::NAME`), its default level, its one-line
//! description, and the `///` doc-comment block that documents it.
//! In addition, when a rule's file defines a `Config` struct paired
//! with a `CONFIG_KEY` constant, the configurable fields (and their
//! per-field doc comments and default expressions) are surfaced too.
//! The output is a single self-contained `index.html` written into
//! the directory passed on the command line.
//!
//! The macro grammar is fixed by `rustc_session::declare_tool_lint!`
//! and the project's convention of placing the doc comment inside
//! the macro braces, so a hand-rolled `syn::parse::Parse` impl is
//! enough — we don't need to invoke the dylint driver or stand up a
//! rustc plugin host just to read these few fields.

use std::{
    collections::HashMap,
    fs,
    ops::Range,
    path::{Path, PathBuf},
    process::ExitCode,
};

use cargo_toml::{Inheritable, Manifest};
use clap::Parser;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use proc_macro2::TokenStream;
use pulldown_cmark::{Event, Options, Tag, TagEnd, html as cmark_html};
use quote::ToTokens;
use strum::{Display, EnumString};
use syn::{
    Attribute, Expr, ExprLit, Ident, ImplItem, Item, Lit, LitStr, Meta, Stmt, Token, Type,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

#[derive(Parser)]
#[clap(about = "Render perfectionist's lint catalogue to a static HTML page")]
struct Cli {
    #[clap(help = "Repository root containing Cargo.toml and src/rules/")]
    root: PathBuf,

    #[clap(help = "Output directory; index.html will be written here")]
    out_dir: PathBuf,
}

fn main() -> ExitCode {
    let Cli { root, out_dir } = Cli::parse();

    let manifest = Manifest::from_path(root.join("Cargo.toml")).expect("failed to read Cargo.toml");
    let crate_version = manifest
        .package
        .as_ref()
        .and_then(|package| match &package.version {
            Inheritable::Set(value) => Some(value.clone()),
            Inheritable::Inherited => None,
        })
        .unwrap_or_else(|| "unknown".to_owned());

    let rules_dir = root.join("src").join("rules");
    let mut rules = collect_rules(&rules_dir);
    rules.sort_by(|a, b| a.namespaced.cmp(&b.namespaced));

    if rules.is_empty() {
        eprintln!("no rules found under {}", rules_dir.display());
        return ExitCode::FAILURE;
    }

    fs::create_dir_all(&out_dir).expect("failed to create output directory");
    let html = render_page(&rules, &crate_version);
    let index_path = out_dir.join("index.html");
    fs::write(&index_path, html).expect("failed to write index.html");

    eprintln!("wrote {} rule(s) to {}", rules.len(), index_path.display());
    ExitCode::SUCCESS
}

/// One lint, in the shape the page needs to render.
struct Rule {
    /// `perfectionist::flat_module_pattern` — used as the anchor.
    namespaced: String,
    /// Default severity of the lint as declared in `declare_tool_lint!`.
    level: Level,
    /// The third positional argument to `declare_tool_lint!`.
    short_desc: String,
    /// The concatenated `///` doc comment lines, in markdown form.
    doc_markdown: String,
    /// Source path relative to the repo root, for cross-linking.
    relative_source: PathBuf,
    /// `Config` struct contents when the rule declares one. `None`
    /// means the rule file has no `Config` / `CONFIG_KEY` pair.
    config: Option<ConfigDoc>,
}

/// The configuration surface of a single rule, as extracted from the
/// rule's `Config` struct, paired `CONFIG_KEY` constant, and (when
/// present) hand-written `impl Default for Config`.
struct ConfigDoc {
    /// The TOML table key, e.g. `perfectionist::flat_module_pattern`.
    /// Read from the file's `CONFIG_KEY` constant verbatim.
    key: String,
    /// Doc comment attached to the `Config` struct itself, useful
    /// for rules with no fields (where it explains why the struct
    /// still exists) or for cross-cutting notes about a rule's
    /// configuration shape.
    struct_doc_markdown: String,
    /// One entry per named field of the `Config` struct.
    fields: Vec<ConfigField>,
}

/// One configurable knob, with the source text of its type and
/// default expression preserved verbatim so the rendered docs match
/// what a reader will see in `src/rules/<rule>.rs`.
struct ConfigField {
    /// Field identifier, e.g. `also_flag`. Matches the TOML key
    /// because every `Config` uses `#[serde(rename_all = "snake_case")]`
    /// and the fields are already named in snake case.
    name: String,
    /// Verbatim source of the field's type (e.g. `Vec<char>`),
    /// sliced out of the rule file using `proc_macro2::Span`
    /// byte ranges.
    type_source: String,
    /// Verbatim source of the field's default expression as it
    /// appears in `impl Default for Config`, or `None` when no
    /// `Default` impl could be located.
    default_source: Option<String>,
    /// Per-field `///` doc comment, in markdown form.
    doc_markdown: String,
}

/// The set of lint levels rustc / Dylint accept as the second
/// positional argument to `declare_tool_lint!`. An unrecognised
/// identifier here is a hard error rather than a silent fallback, so
/// future additions to rustc's level list are caught immediately.
#[derive(Debug, Clone, Copy, EnumString, Display)]
enum Level {
    Warn,
    Deny,
    Forbid,
    Allow,
}

impl Level {
    fn css_class(self) -> &'static str {
        match self {
            Level::Warn => "level-warn",
            Level::Deny | Level::Forbid => "level-deny",
            Level::Allow => "level-allow",
        }
    }
}

fn collect_rules(rules_dir: &Path) -> Vec<Rule> {
    let entries = fs::read_dir(rules_dir).expect("failed to read src/rules/");
    let mut rules = Vec::new();
    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let Some(rule) = extract_rule(&path) else {
            continue;
        };
        rules.push(rule);
    }
    rules
}

fn extract_rule(source_path: &Path) -> Option<Rule> {
    let source = fs::read_to_string(source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let file = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", source_path.display()));
    let macro_item = file.items.iter().find_map(|item| match item {
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
    })?;

    let declaration = syn::parse2::<DeclareToolLint>(macro_item.clone()).unwrap_or_else(|error| {
        panic!(
            "failed to parse declare_tool_lint! body in {}: {error}",
            source_path.display()
        )
    });

    let namespaced = format!(
        "perfectionist::{}",
        declaration.name.to_string().to_ascii_lowercase()
    );
    let doc_markdown = doc_attrs_to_markdown(&declaration.attrs);
    let relative_source = source_path
        .components()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let level_ident = declaration.level.to_string();
    let level: Level = level_ident.parse().unwrap_or_else(|_| {
        panic!(
            "unknown lint level `{level_ident}` in {}",
            source_path.display()
        )
    });

    let config = extract_config(&file, &source);

    Some(Rule {
        namespaced,
        level,
        short_desc: declaration.desc.value(),
        doc_markdown,
        relative_source,
        config,
    })
}

/// Locate the rule's `Config` struct, its `CONFIG_KEY` constant, and
/// the optional `impl Default for Config` block, and bundle them
/// into a `ConfigDoc`. Returns `None` when either the constant or
/// the struct is missing — both are mandatory for a rule to be
/// considered "configurable" by `dylint.toml`.
fn extract_config(file: &syn::File, source: &str) -> Option<ConfigDoc> {
    let key = file.items.iter().find_map(|item| match item {
        Item::Const(item_const) if item_const.ident == "CONFIG_KEY" => match &*item_const.expr {
            Expr::Lit(ExprLit {
                lit: Lit::Str(literal),
                ..
            }) => Some(literal.value()),
            _ => None,
        },
        _ => None,
    })?;
    let config_struct = file.items.iter().find_map(|item| match item {
        Item::Struct(item_struct) if item_struct.ident == "Config" => Some(item_struct),
        _ => None,
    })?;
    let struct_doc_markdown = doc_attrs_to_markdown(&config_struct.attrs);
    let defaults = extract_config_defaults(file, source);

    let fields = match &config_struct.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                let name = field
                    .ident
                    .as_ref()
                    .expect("named field always has an ident")
                    .to_string();
                let type_source = span_text(source, field.ty.span())
                    .unwrap_or_else(|| fallback_type_text(&field.ty));
                let default_source = defaults.get(&name).cloned();
                let doc_markdown = doc_attrs_to_markdown(&field.attrs);
                ConfigField {
                    name,
                    type_source,
                    default_source,
                    doc_markdown,
                }
            })
            .collect(),
        _ => Vec::new(),
    };

    Some(ConfigDoc {
        key,
        struct_doc_markdown,
        fields,
    })
}

/// Pull each field's default expression out of `impl Default for
/// Config`. Recognises the conventional shape
/// `fn default() -> Self { Self { field: expr, ... } }` (with or
/// without an explicit `return`); anything more exotic yields an
/// empty map and the rendered docs simply omit the default column.
fn extract_config_defaults(file: &syn::File, source: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(impl_item) = file.items.iter().find_map(|item| match item {
        Item::Impl(item_impl)
            if item_impl.trait_.as_ref().is_some_and(|(_, path, _)| {
                path.segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Default")
            }) && matches!(
                &*item_impl.self_ty,
                Type::Path(type_path)
                    if type_path
                        .path
                        .segments
                        .last()
                        .is_some_and(|segment| segment.ident == "Config")
            ) =>
        {
            Some(item_impl)
        }
        _ => None,
    }) else {
        return out;
    };

    let Some(default_fn) = impl_item.items.iter().find_map(|item| match item {
        ImplItem::Fn(item_fn) if item_fn.sig.ident == "default" => Some(item_fn),
        _ => None,
    }) else {
        return out;
    };

    let Some(struct_expr) = default_fn
        .block
        .stmts
        .last()
        .and_then(|stmt| match stmt {
            Stmt::Expr(expr, _) => Some(expr),
            _ => None,
        })
        .and_then(|expr| match expr {
            Expr::Struct(struct_expr) => Some(struct_expr),
            Expr::Return(return_expr) => match return_expr.expr.as_deref() {
                Some(Expr::Struct(struct_expr)) => Some(struct_expr),
                _ => None,
            },
            _ => None,
        })
    else {
        return out;
    };

    for field_value in &struct_expr.fields {
        let syn::Member::Named(ident) = &field_value.member else {
            continue;
        };
        let text = span_text(source, field_value.expr.span())
            .unwrap_or_else(|| field_value.expr.to_token_stream().to_string());
        out.insert(ident.to_string(), text);
    }
    out
}

/// Slice the verbatim source bytes covered by `span`. Falls back to
/// `None` when the span has no usable byte range (which happens for
/// synthetic spans produced by macro expansion, not for code that
/// originated in a file we just parsed — but the type system makes
/// us account for the case anyway).
fn span_text(source: &str, span: proc_macro2::Span) -> Option<String> {
    let Range { start, end } = span.byte_range();
    if end <= start || end > source.len() {
        return None;
    }
    Some(source[start..end].to_owned())
}

/// Best-effort stringifier for a `syn::Type` when no source span is
/// available. The output uses `quote`'s default token spacing, which
/// is less pretty than the original source but always parseable.
fn fallback_type_text(ty: &Type) -> String {
    ty.to_token_stream().to_string()
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

fn doc_attrs_to_markdown(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &meta.value
        else {
            continue;
        };
        // `/// foo` lexes as `#[doc = " foo"]`; drop the single
        // convention-space so the markdown round-trips cleanly.
        let raw = s.value();
        let trimmed = raw.strip_prefix(' ').unwrap_or(&raw).to_owned();
        lines.push(trimmed);
    }
    lines.join("\n")
}

fn render_page(rules: &[Rule], crate_version: &str) -> String {
    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "perfectionist lints" }
                style { (PreEscaped(STYLE)) }
            }
            body {
                h1 { "perfectionist lints" }
                div.banner {
                    "Showing development docs from " code { "master" } ". "
                    "Latest released version: " code { (crate_version) } "."
                }
                p {
                    "perfectionist is a Dylint plugin; see the "
                    a href="https://github.com/KSXGitHub/perfectionist" { "README" }
                    " for setup. Lint-control attributes use the "
                    code { "perfectionist::" } " namespace."
                }
                h2 { "Index" }
                table.index {
                    thead {
                        tr {
                            th { "Lint" }
                            th { "Default" }
                            th { "Description" }
                        }
                    }
                    tbody {
                        @for rule in rules {
                            tr {
                                td {
                                    a href={ "#" (anchor_for(&rule.namespaced)) } {
                                        code { (rule.namespaced) }
                                    }
                                }
                                td { (level_badge(rule.level)) }
                                td {
                                    (PreEscaped(markdown_inline_to_html(&rule.short_desc)))
                                }
                            }
                        }
                    }
                }
                h2 { "Rules" }
                @for rule in rules {
                    (rule_article(rule))
                }
                footer {
                    "Generated from " code { "src/rules/" } "."
                }
            }
        }
    };
    markup.into_string()
}

fn rule_article(rule: &Rule) -> Markup {
    let source_path = rule.relative_source.display().to_string();
    let source_url =
        format!("https://github.com/KSXGitHub/perfectionist/blob/master/{source_path}");
    html! {
        article.rule id=(anchor_for(&rule.namespaced)) {
            h2 { code { (rule.namespaced) } }
            p {
                (level_badge(rule.level))
                (PreEscaped(markdown_inline_to_html(&rule.short_desc)))
            }
            (PreEscaped(markdown_to_html(&rule.doc_markdown)))
            @if let Some(config) = &rule.config {
                (config_section(config))
            }
            p.source {
                "Source: "
                a href=(source_url) { code { (source_path) } }
            }
        }
    }
}

/// Render the per-rule configuration block. When the `Config`
/// struct has no fields, the block is still emitted so readers
/// can see that the rule is intentionally non-configurable (and,
/// where the struct itself carries a doc comment, why).
fn config_section(config: &ConfigDoc) -> Markup {
    html! {
        h3 { "Configuration" }
        p {
            "Configure via " code { "dylint.toml" } " under "
            code { "[\"" (config.key) "\"]" } "."
        }
        @if !config.struct_doc_markdown.is_empty() {
            (PreEscaped(markdown_to_html(&config.struct_doc_markdown)))
        }
        @if config.fields.is_empty() {
            @if config.struct_doc_markdown.is_empty() {
                p { em { "No configurable options." } }
            }
        } @else {
            dl.config {
                @for field in &config.fields {
                    dt {
                        code.config-key { (field.name) }
                        " : "
                        code.config-type { (field.type_source) }
                        @if let Some(default) = &field.default_source {
                            " "
                            span.config-default {
                                "(default: " code { (default) } ")"
                            }
                        }
                    }
                    dd {
                        @if field.doc_markdown.is_empty() {
                            p { em { "Undocumented." } }
                        } @else {
                            (PreEscaped(markdown_to_html(&field.doc_markdown)))
                        }
                    }
                }
            }
        }
    }
}

fn level_badge(level: Level) -> Markup {
    let class = format!("level {}", level.css_class());
    html! {
        span class=(class) { (level.to_string()) }
    }
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = pulldown_cmark::Parser::new_ext(markdown, options);
    let mut buffer = String::new();
    cmark_html::push_html(&mut buffer, parser);
    buffer
}

/// Render a short, single-line snippet of markdown without the
/// outer `<p>…</p>` wrapper that block-level rendering inserts. Used
/// for table cells and inline headings where backticks should still
/// turn into `<code>`.
fn markdown_inline_to_html(markdown: &str) -> String {
    let parser = pulldown_cmark::Parser::new(markdown).filter(|event| {
        !matches!(
            event,
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph)
        )
    });
    let mut buffer = String::new();
    cmark_html::push_html(&mut buffer, parser);
    buffer.trim_end().to_owned()
}

fn anchor_for(namespaced: &str) -> String {
    namespaced.replace("::", "-")
}

const STYLE: &str = include_str!("style.css");
