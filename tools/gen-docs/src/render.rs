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
/// Without JS the toggle can't do anything (no click handler, no
/// open/close, no scroll lock), so hide it. No-JS readers still
/// have the full index table at the top of the page for navigation.
const NAV_TOGGLE_NOSCRIPT_CSS: &str = ".nav-toggle{display:none!important;}";

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

/// Collapsible navigation drawer. The visible toggle is a plain
/// `<button>` rather than a `<summary>` inside `<details>` so the
/// open/closed state can be driven by `aria-expanded` from JS,
/// and the menu is a sibling `<nav>` so the adjacent-sibling
/// selector `.nav-toggle[aria-expanded="true"] + .nav-sidebar`
/// drives display without scripting reaching into the nav.
///
/// On narrow viewports the nav becomes a full-screen overlay
/// (the JS also locks body scroll while it's open, which stops
/// the mobile URL bar from collapsing under it and keeps every
/// `position: fixed` element steady). The close (✕) button lives
/// inside the overlay in normal flow rather than as a fixed-
/// position sibling, so it's never affected by visual-viewport
/// quirks even on browsers where `position: fixed` drifts with
/// the URL bar.
fn nav_drawer(rules: &[Rule]) -> Markup {
    html! {
        button.nav-toggle
            type="button"
            aria-controls="nav-sidebar"
            aria-expanded="false"
            aria-label="Toggle navigation"
            title="Toggle navigation" {}
        nav.nav-sidebar id="nav-sidebar" aria-label="Lint rules" {
            div.nav-sidebar-header {
                button.nav-sidebar-close
                    type="button"
                    aria-label="Close navigation"
                    title="Close navigation" { "\u{2715}" }
                a.nav-sidebar-title href="#catalogue" { "perfectionist lints" }
            }
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

    /// Locate the sidebar's `<ul>` and return the slice spanning
    /// just its contents. The narrow-viewport CSS selector
    /// `.nav-toggle[aria-expanded="true"] + .nav-sidebar` depends
    /// on adjacent-sibling ordering, so tests that want to count
    /// or inspect sidebar entries should do so against this slice
    /// — not the whole page, where the index table and rule
    /// articles emit substrings (`href="#perfectionist-*"`,
    /// `<code>name</code>`, `id="perfectionist-*"`) that overlap
    /// the sidebar's markup.
    fn sidebar_list(html: &str) -> &str {
        let open = "<ul class=\"nav-sidebar-list\">";
        let close = "</ul>";
        let start = html.find(open).expect("sidebar <ul> not rendered") + open.len();
        let end = start
            + html[start..]
                .find(close)
                .expect("sidebar <ul> unterminated");
        &html[start..end]
    }

    #[test]
    fn page_emits_nav_toggle_button_and_sibling_nav() {
        let html = render_page(&[fake_rule("alpha")], &fake_context());
        // Toggle is a plain <button> driven by `aria-expanded`, not
        // a <summary>/<details>, so JS can control open state and
        // the CSS adjacent-sibling selector is keyed off the same
        // attribute. The button starts in the closed state.
        assert!(html.contains("<button class=\"nav-toggle\""));
        assert!(html.contains("aria-controls=\"nav-sidebar\""));
        assert!(html.contains("aria-expanded=\"false\""));
        assert!(html.contains("aria-label=\"Toggle navigation\""));
        assert!(html.contains("aria-label=\"Lint rules\""));
        // The narrow-viewport CSS uses
        // `.nav-toggle[aria-expanded="true"] + .nav-sidebar`, so
        // the <nav> must be the immediate next sibling of
        // </button>. A refactor that nested the <nav> inside the
        // button or inserted another element between them would
        // break the overlay on narrow viewports; pin the literal
        // boundary.
        assert!(
            html.contains("</button><nav class=\"nav-sidebar\" id=\"nav-sidebar\""),
            "expected </button> to be immediately followed by the <nav class=\"nav-sidebar\"> sibling",
        );
        // The close (✕) button lives inside the overlay so the
        // drawer can be dismissed without any fixed-position
        // element acting as both opener and closer. It must also
        // come *before* the title in DOM order: the hamburger
        // sits at the top-left, so on close the user's finger is
        // already there — putting the ✕ next to a tappable rule
        // link would invite a misclick on close.
        let header_start = html
            .find("<div class=\"nav-sidebar-header\">")
            .expect("nav-sidebar-header missing");
        let close_pos = html[header_start..]
            .find("<button class=\"nav-sidebar-close\"")
            .expect("nav-sidebar-close missing");
        let title_pos = html[header_start..]
            .find("<a class=\"nav-sidebar-title\"")
            .expect("nav-sidebar-title missing");
        assert!(
            close_pos < title_pos,
            "close button must appear before title in DOM order",
        );
    }

    #[test]
    fn page_emits_one_sidebar_entry_per_rule_with_anchor_links() {
        let rules = [fake_rule("alpha"), fake_rule("beta_gamma")];
        let html = render_page(&rules, &fake_context());
        let sidebar = sidebar_list(&html);
        // One <li> per rule, scoped to the sidebar's <ul> so that
        // the index table's rows and the rule articles can't make
        // these assertions pass on their own.
        assert_eq!(sidebar.matches("<li>").count(), rules.len());
        assert!(
            sidebar.contains("<li><a href=\"#perfectionist-alpha\"><code>alpha</code></a></li>")
        );
        assert!(sidebar.contains(
            "<li><a href=\"#perfectionist-beta_gamma\"><code>beta_gamma</code></a></li>"
        ));
        // The rule articles those anchors resolve to must exist
        // outside the sidebar slice, otherwise the sidebar links
        // dangle.
        assert!(html.contains("id=\"perfectionist-alpha\""));
        assert!(html.contains("id=\"perfectionist-beta_gamma\""));
    }

    #[test]
    fn page_inlines_nav_toggle_script_and_noscript_fallback() {
        let html = render_page(&[fake_rule("only")], &fake_context());
        // Assert the full script body appears verbatim inside a
        // <script> tag. `<script>` on its own would match an empty
        // tag; substrings like `IntersectionObserver` also appear
        // in the CSS comments that ship in the same HTML. The
        // wrapped-content check pins both that the script is
        // present and that NAV_TOGGLE_SCRIPT was the body.
        let wrapped = format!("<script>{NAV_TOGGLE_SCRIPT}</script>");
        assert!(
            html.contains(&wrapped),
            "expected NAV_TOGGLE_SCRIPT to be inlined verbatim inside a <script> tag",
        );
        // The toggle does nothing without JS, so the <noscript>
        // override in <head> hides it; no-JS readers fall back to
        // the full index table near the top of the page.
        let noscript = format!("<noscript><style>{NAV_TOGGLE_NOSCRIPT_CSS}</style></noscript>");
        assert!(html.contains(&noscript));
    }
}
