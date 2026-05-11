//! Generate the static documentation site for perfectionist's
//! implemented lints.
//!
//! Reads each `src/rules/*.rs` (skipping `mod.rs`-style index files
//! that don't declare a lint), locates the `declare_tool_lint!`
//! invocation, and pulls four things out of it: the lint identifier
//! (`pub perfectionist::NAME`), its default level, its one-line
//! description, and the `///` doc-comment block that documents it.
//! The output is a single self-contained `index.html` written into
//! the directory passed on the command line.
//!
//! The macro grammar is fixed by `rustc_session::declare_tool_lint!`
//! and the project's convention of placing the doc comment inside
//! the macro braces, so a hand-rolled `syn::parse::Parse` impl is
//! enough — we don't need to invoke the dylint driver or stand up a
//! rustc plugin host just to read these few fields.

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::LazyLock,
};

use cargo_toml::{Inheritable, Manifest};
use clap::Parser;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use proc_macro2::TokenStream;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Tag, TagEnd, html as cmark_html};
use strum::{Display, EnumString};
use syn::{
    Attribute, Expr, ExprLit, Ident, Item, Lit, LitStr, Meta, Token,
    parse::{Parse, ParseStream},
};
use syntect::{
    highlighting::{Theme, ThemeSet},
    html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style},
    parsing::SyntaxSet,
    util::LinesWithEndings,
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

    Some(Rule {
        namespaced,
        level,
        short_desc: declaration.desc.value(),
        doc_markdown,
        relative_source,
    })
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
                style { (PreEscaped(STYLE)) (PreEscaped(highlight_css())) }
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
            p.source {
                "Source: "
                a href=(source_url) { code { (source_path) } }
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
    let events = highlight_code_blocks(parser);
    let mut buffer = String::new();
    cmark_html::push_html(&mut buffer, events.into_iter());
    buffer
}

/// Walk pulldown-cmark events, replacing fenced code blocks with
/// pre-highlighted HTML produced by `syntect`. The first comma-
/// separated token of the fence's info string is the language tag
/// (so `rust,ignore` is treated as Rust); unknown or missing tags
/// fall back to plain text.
fn highlight_code_blocks<'a>(parser: impl Iterator<Item = Event<'a>>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut current_lang: Option<String> = None;
    let mut code_buffer = String::new();
    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let lang = info
                    .split(',')
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                if let Some(lang) = lang {
                    current_lang = Some(lang);
                    code_buffer.clear();
                } else {
                    // Untagged fenced blocks have no language to highlight;
                    // let pulldown-cmark emit them as plain `<pre><code>`.
                    out.push(Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))));
                }
            }
            Event::End(TagEnd::CodeBlock) if current_lang.is_some() => {
                let lang = current_lang.take().expect("guarded above");
                let html = highlight_to_html(&code_buffer, &lang);
                out.push(Event::Html(CowStr::Boxed(html.into_boxed_str())));
            }
            Event::Text(text) if current_lang.is_some() => {
                code_buffer.push_str(&text);
            }
            other => out.push(other),
        }
    }
    out
}

fn highlight_to_html(code: &str, lang: &str) -> String {
    let syntax_set: &SyntaxSet = &SYNTAX_SET;
    let extension = match lang {
        "rust" => "rs",
        other => other,
    };
    let syntax = syntax_set
        .find_syntax_by_extension(extension)
        .or_else(|| syntax_set.find_syntax_by_name(lang))
        .or_else(|| syntax_set.find_syntax_by_token(lang))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);
    for line in LinesWithEndings::from(code) {
        // The classed generator only fails on malformed regex
        // matches, which can't happen against the bundled syntaxes.
        generator
            .parse_html_for_line_which_includes_newline(line)
            .expect("classed highlighting of bundled syntax should not fail");
    }
    let body = generator.finalize();
    let class_attr = lang_class_attr(lang);
    format!("<pre><code{class_attr}>{body}</code></pre>\n")
}

/// Render the `language-…` class attribute for a code block, but
/// only when the fence's language tag is made of characters that
/// are unambiguously safe to drop into an HTML attribute. Anything
/// outside the allow-list yields an empty attribute — the page
/// still renders correctly because the inline `<span class>`es
/// generated by `syntect` carry the actual highlighting, and the
/// `language-…` class is purely decorative here.
fn lang_class_attr(lang: &str) -> String {
    let safe = !lang.is_empty()
        && lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '#' | '.'));
    if safe {
        format!(" class=\"language-{lang}\"")
    } else {
        String::new()
    }
}

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    ThemeSet::load_defaults()
        .themes
        .remove("InspiredGitHub")
        .expect("InspiredGitHub theme is bundled with syntect")
});

fn highlight_css() -> String {
    css_for_theme_with_class_style(&THEME, ClassStyle::Spaced)
        .expect("generating CSS for a bundled theme should not fail")
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
