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
};

use cargo_toml::{Inheritable, Manifest};
use clap::Parser;
use proc_macro2::TokenStream;
use pulldown_cmark::{Event, Options, Tag, TagEnd, html as cmark_html};
use syn::{
    Attribute, Expr, ExprLit, Ident, Item, Lit, LitStr, Meta, Token,
    parse::{Parse, ParseStream},
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
    /// `Warn` / `Deny` / `Allow` / ...
    level: String,
    /// The third positional argument to `declare_tool_lint!`.
    short_desc: String,
    /// The concatenated `///` doc comment lines, in markdown form.
    doc_markdown: String,
    /// Source path relative to the repo root, for cross-linking.
    relative_source: PathBuf,
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
    let source = fs::read_to_string(source_path).expect("failed to read rule source");
    let file = syn::parse_file(&source).expect("failed to parse rule source");
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

    let declaration = syn::parse2::<DeclareToolLint>(macro_item.clone())
        .expect("failed to parse declare_tool_lint! body");

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

    Some(Rule {
        namespaced,
        level: declaration.level.to_string(),
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
    let mut index_rows = String::new();
    for rule in rules {
        let level_class = level_css_class(&rule.level);
        let short_desc_html = markdown_inline_to_html(&rule.short_desc);
        let namespaced_html = escape_html(&rule.namespaced);
        let anchor = anchor_for(&rule.namespaced);
        index_rows.push_str(&format!(
            "      <tr><td><a href=\"#{anchor}\"><code>{namespaced_html}</code></a></td>\
             <td><span class=\"level {level_class}\">{level}</span></td>\
             <td>{short_desc_html}</td></tr>\n",
            level = rule.level,
        ));
    }

    let mut sections = String::new();
    for rule in rules {
        let anchor = anchor_for(&rule.namespaced);
        let namespaced_html = escape_html(&rule.namespaced);
        let short_desc_html = markdown_inline_to_html(&rule.short_desc);
        let level_class = level_css_class(&rule.level);
        let body_html = markdown_to_html(&rule.doc_markdown);
        let source_html = escape_html(&rule.relative_source.display().to_string());
        let source_path = rule.relative_source.display().to_string();
        sections.push_str(&format!(
            "    <article class=\"rule\" id=\"{anchor}\">\n\
             \x20     <h2><code>{namespaced_html}</code></h2>\n\
             \x20     <p><span class=\"level {level_class}\">{level}</span> \
             &mdash; {short_desc_html}</p>\n\
             {body_html}\n\
             \x20     <p class=\"source\">Source: \
             <a href=\"https://github.com/KSXGitHub/perfectionist/blob/master/{source_path}\">\
             <code>{source_html}</code></a></p>\n\
             \x20   </article>\n",
            level = rule.level,
        ));
    }

    let version_html = escape_html(crate_version);

    format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
\x20 <meta charset=\"utf-8\">\n\
\x20 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
\x20 <title>perfectionist lints</title>\n\
\x20 <style>{STYLE}</style>\n\
</head>\n\
<body>\n\
\x20 <h1>perfectionist lints</h1>\n\
\x20 <div class=\"banner\">Showing development docs from <code>master</code>. \
Latest released version: <code>{version_html}</code>.</div>\n\
\x20 <p>perfectionist is a Dylint plugin; see the \
<a href=\"https://github.com/KSXGitHub/perfectionist\">README</a> for setup. \
Lint-control attributes use the <code>perfectionist::</code> namespace, e.g. \
<code>#[allow(perfectionist::flat_module_pattern)]</code>.</p>\n\
\x20 <h2>Index</h2>\n\
\x20 <table class=\"index\">\n\
\x20   <thead><tr><th>Lint</th><th>Default</th><th>Description</th></tr></thead>\n\
\x20   <tbody>\n\
{index_rows}\
\x20   </tbody>\n\
\x20 </table>\n\
\x20 <h2>Rules</h2>\n\
{sections}\
\x20 <footer>Generated from <code>src/rules/</code>.</footer>\n\
</body>\n\
</html>\n",
        STYLE = STYLE,
    )
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

fn level_css_class(level: &str) -> &'static str {
    match level {
        "Warn" => "level-warn",
        "Deny" | "Forbid" => "level-deny",
        "Allow" => "level-allow",
        _ => "level-other",
    }
}

fn escape_html(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for char in input.chars() {
        match char {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            other => output.push(other),
        }
    }
    output
}

const STYLE: &str = "
body { font-family: -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
       max-width: 820px; margin: 2rem auto; padding: 0 1rem; line-height: 1.55;
       color: #1f2328; }
h1 { border-bottom: 1px solid #d0d7de; padding-bottom: .3rem; }
h2 { margin-top: 2.2rem; }
.banner { background: #fff8c5; border-left: 4px solid #d4a72c;
          padding: .75rem 1rem; margin: 1rem 0 1.5rem; font-size: .9rem; }
table.index { width: 100%; border-collapse: collapse; margin-bottom: 2rem; }
table.index td, table.index th { text-align: left; padding: .4rem .6rem;
                                 border-bottom: 1px solid #eaeef2;
                                 vertical-align: top; }
table.index th { background: #f6f8fa; }
.level { font-family: ui-monospace, SFMono-Regular, monospace; font-size: .8rem;
         padding: .1rem .45rem; border-radius: 3px; }
.level-warn { background: #fff8c5; }
.level-deny { background: #ffebe9; }
.level-allow { background: #eaeef2; }
.level-other { background: #ddf4ff; }
article.rule { margin-bottom: 3rem; padding-top: 1rem;
               border-top: 1px solid #eaeef2; }
article.rule h2 { font-family: ui-monospace, SFMono-Regular, monospace; }
article.rule .source { font-size: .85rem; color: #57606a; }
pre, code { font-family: ui-monospace, SFMono-Regular, monospace; }
pre { background: #f6f8fa; padding: .75rem 1rem; border-radius: 6px;
      overflow-x: auto; font-size: .88rem; }
code { background: #f6f8fa; padding: .1rem .35rem; border-radius: 3px;
       font-size: .9rem; }
pre code { background: none; padding: 0; font-size: 1rem; }
footer { margin-top: 4rem; padding-top: 1rem; border-top: 1px solid #eaeef2;
         font-size: .85rem; color: #57606a; }
a { color: #0969da; }
a:hover { text-decoration: underline; }
";
