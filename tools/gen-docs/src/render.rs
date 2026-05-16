//! Render the collected [`Rule`]s into a single self-contained
//! `index.html`. The page is intentionally one file: every reader of
//! the catalogue should be able to ctrl-F across every rule's prose
//! without page loads, and the static-site host (GitHub Pages) is
//! happiest with a directory it can serve verbatim.

pub(crate) mod config;
pub(crate) mod markdown;

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::model::{DefaultState, NAMESPACE, RenderContext, Rule};
use crate::render::config::config_section;
use crate::render::markdown::{HIGHLIGHT_CSS, markdown_inline_to_html, markdown_to_html};

const STYLE: &str = include_str!("style.css");
const NAV_TOGGLE_SCRIPT: &str = include_str!("nav_toggle.js");
const NAV_TOGGLE_NOSCRIPT_CSS: &str =
    ".nav-toggle{opacity:1!important;visibility:visible!important;}";

pub(crate) fn render_page(rules: &[Rule], context: &RenderContext<'_>) -> String {
    let RenderContext {
        crate_version,
        git_ref,
        commit_sha,
        repo_url,
    } = *context;
    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="description" content="Catalogue of perfectionist's lints — a Dylint plugin adding stylistic and correctness lints for Rust projects.";
                title {
                    "perfectionist lints"
                    @if git_ref != "master" { " — " (git_ref) }
                }
                style { (PreEscaped(STYLE)) (PreEscaped(&*HIGHLIGHT_CSS)) }
                noscript {
                    style { (PreEscaped(NAV_TOGGLE_NOSCRIPT_CSS)) }
                }
            }
            body {
                h1 id="catalogue" { "perfectionist lints" }
                (nav_drawer(rules))
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
                    code { (NAMESPACE) } " namespace."
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
                                td { (state_badge(rule.default_state)) }
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
                    " at " code { (commit_sha) } "."
                }
                script { (PreEscaped(NAV_TOGGLE_SCRIPT)) }
            }
        }
    };
    markup.into_string()
}

/// Collapsible navigation drawer. Uses `<details>` so the toggle is
/// a native disclosure widget — keyboard-operable and screen-reader-
/// announced without scripting — and skins it as a hamburger button.
/// The `<nav>` deliberately lives outside the `<details>` and rides
/// the adjacent-sibling selector: putting it inside would trip the
/// modern UA stylesheet's `::details-content { content-visibility:
/// hidden }` rule when the details is closed, which can't be
/// overridden from the descendant. As a sibling, the nav's
/// visibility is plain author CSS — reliable across browsers — and
/// the wide-viewport media query can force-show it unconditionally.
fn nav_drawer(rules: &[Rule]) -> Markup {
    html! {
        details.nav-drawer {
            summary.nav-toggle
                aria-label="Toggle navigation"
                aria-controls="nav-sidebar"
                title="Toggle navigation" {}
        }
        nav.nav-sidebar id="nav-sidebar" aria-label="Lint rules" {
            a.nav-sidebar-title href="#catalogue" { "perfectionist lints" }
            ul.nav-sidebar-list {
                @for rule in rules {
                    li {
                        a href={ "#" (anchor_for(&rule.namespaced)) } {
                            code { (unnamespaced(&rule.namespaced)) }
                        }
                    }
                }
            }
        }
    }
}

fn rule_article(rule: &Rule, context: &RenderContext<'_>) -> Markup {
    let RenderContext {
        commit_sha,
        repo_url,
        ..
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
    let source_url = format!("{repo_url}/blob/{commit_sha}/{source_path}");
    html! {
        article.rule id=(anchor_for(&rule.namespaced)) {
            h2 {
                code {
                    span.lint-prefix { (NAMESPACE) }
                    span.lint-name { (unnamespaced(&rule.namespaced)) }
                }
                a.rule-jump-link href="#catalogue" aria-label="Back to catalogue" { "↑ top" }
            }
            p {
                (state_badge(rule.default_state))
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

fn state_badge(default_state: DefaultState) -> Markup {
    html! {
        span class=(default_state.css_class()) { (default_state.word()) }
    }
}

fn anchor_for(namespaced: &str) -> String {
    namespaced.replace("::", "-")
}

fn unnamespaced(namespaced: &str) -> &str {
    namespaced.strip_prefix(NAMESPACE).unwrap_or(namespaced)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fake_rule(name: &str) -> Rule {
        Rule {
            namespaced: format!("perfectionist::{name}"),
            default_state: DefaultState::Enabled,
            short_desc: format!("demo rule {name}"),
            doc_markdown: "### What it does\nDoes a demo.".to_owned(),
            relative_source: PathBuf::from(format!("src/rules/{name}.rs")),
            config: None,
        }
    }

    fn fake_context() -> RenderContext<'static> {
        RenderContext {
            crate_version: "0.0.0-test",
            git_ref: "master",
            commit_sha: "0000000000000000000000000000000000000000",
            repo_url: "https://example.invalid/perfectionist",
        }
    }

    #[test]
    fn page_emits_nav_drawer_details_and_sibling_nav() {
        let html = render_page(&[fake_rule("alpha")], &fake_context());
        assert!(html.contains("<details class=\"nav-drawer\">"));
        assert!(html.contains("class=\"nav-toggle\""));
        assert!(html.contains("aria-label=\"Toggle navigation\""));
        assert!(html.contains("aria-controls=\"nav-sidebar\""));
        assert!(html.contains("id=\"nav-sidebar\""));
        assert!(html.contains("aria-label=\"Lint rules\""));
    }

    #[test]
    fn page_emits_one_sidebar_entry_per_rule_with_anchor_links() {
        let rules = [fake_rule("alpha"), fake_rule("beta_gamma")];
        let html = render_page(&rules, &fake_context());
        // Each <li> in the sidebar should contain a link to the
        // rule's article anchor; the link text is the unnamespaced
        // rule name in a <code>.
        assert!(html.contains("href=\"#perfectionist-alpha\""));
        assert!(html.contains("<code>alpha</code>"));
        assert!(html.contains("href=\"#perfectionist-beta_gamma\""));
        assert!(html.contains("<code>beta_gamma</code>"));
        // And the rule articles those anchors point at must exist.
        assert!(html.contains("id=\"perfectionist-alpha\""));
        assert!(html.contains("id=\"perfectionist-beta_gamma\""));
    }

    #[test]
    fn page_inlines_nav_toggle_script_and_noscript_fallback() {
        let html = render_page(&[fake_rule("only")], &fake_context());
        assert!(html.contains("<script>"));
        assert!(html.contains("IntersectionObserver"));
        // <noscript> in <head> reverts the toggle to always-visible
        // for the explicit-noscript case.
        assert!(html.contains("<noscript><style>"));
        assert!(html.contains(NAV_TOGGLE_NOSCRIPT_CSS));
    }
}
