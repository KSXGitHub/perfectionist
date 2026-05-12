//! Render the collected [`Rule`]s into a single self-contained
//! `index.html`. The page is intentionally one file: every reader of
//! the catalogue should be able to ctrl-F across every rule's prose
//! without page loads, and the static-site host (GitHub Pages) is
//! happiest with a directory it can serve verbatim.

pub(crate) mod config;
pub(crate) mod markdown;

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::model::{Level, RenderContext, Rule};
use crate::render::config::config_section;
use crate::render::markdown::{HIGHLIGHT_CSS, markdown_inline_to_html, markdown_to_html};

const STYLE: &str = include_str!("style.css");

pub(crate) fn render_page(rules: &[Rule], context: &RenderContext<'_>) -> String {
    let RenderContext {
        crate_version,
        git_ref,
        repo_url,
    } = *context;
    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title {
                    "perfectionist lints"
                    @if git_ref != "master" { " — " (git_ref) }
                }
                style { (PreEscaped(STYLE)) (PreEscaped(&*HIGHLIGHT_CSS)) }
            }
            body {
                h1 { "perfectionist lints" }
                div.banner {
                    @if git_ref == "master" {
                        "Showing development docs from " code { "master" } ". "
                        "Latest released version: " code { (crate_version) } "."
                    } @else {
                        "Showing docs for " code { (git_ref) } "."
                    }
                }
                p {
                    "perfectionist is a Dylint plugin; see the "
                    a href=(repo_url) { "README" }
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
                                        code { (unnamespaced(&rule.namespaced)) }
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
                    (rule_article(rule, context))
                }
                footer {
                    "Generated from " code { "src/rules/" }
                    " at " code { (git_ref) } "."
                }
            }
        }
    };
    markup.into_string()
}

fn rule_article(rule: &Rule, context: &RenderContext<'_>) -> Markup {
    let RenderContext {
        git_ref, repo_url, ..
    } = *context;
    // Build the URL-friendly path by joining components with `/`
    // instead of `Path::display`, which uses the host's native
    // separator and would emit `\` on Windows.
    let source_path = rule
        .relative_source
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let source_url = format!("{repo_url}/blob/{git_ref}/{source_path}");
    html! {
        article.rule id=(anchor_for(&rule.namespaced)) {
            h2 {
                code {
                    span.lint-prefix { "perfectionist::" }
                    span.lint-name { (unnamespaced(&rule.namespaced)) }
                }
            }
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

fn level_badge(level: Level) -> Markup {
    let class = format!("level {}", level.css_class());
    html! {
        span class=(class) { (level.to_string()) }
    }
}

fn anchor_for(namespaced: &str) -> String {
    namespaced.replace("::", "-")
}

fn unnamespaced(namespaced: &str) -> &str {
    namespaced
        .strip_prefix("perfectionist::")
        .unwrap_or(namespaced)
}
