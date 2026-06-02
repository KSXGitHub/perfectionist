//! Render the collected [`Rule`]s into `index.html` plus the sibling
//! assets it links — the stylesheets and the navigation script, each
//! written as a standalone file. The page itself stays a single
//! document so every reader of the catalogue can ctrl-F across every
//! rule's prose without page loads; the CSS and JS live in their own
//! files (rather than inlined) so each can be edited and cached
//! independently and so future changes can add or split sheets without
//! reflowing one monolith. GitHub Pages serves the whole output
//! directory verbatim.

pub(crate) mod config;
pub(crate) mod markdown;

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::model::{DefaultState, NAMESPACE, RenderContext, Rule};
use crate::render::config::config_section;
use crate::render::markdown::{markdown_inline_to_html, markdown_to_html};

/// The static stylesheets, each emitted as its own file beside
/// `index.html` and linked with a dedicated `<link rel="stylesheet">`.
/// They are deliberately *not* concatenated into one sheet: keeping
/// them separate lets each be edited and cached on its own and leaves
/// room for future sheets without reflowing a monolith. The slice
/// order is the cascade order the page links them in.
pub(crate) const STYLESHEETS: &[(&str, &str)] = &[
    ("base.css", include_str!("style/base.css")),
    ("nav.css", include_str!("style/nav.css")),
    ("rules.css", include_str!("style/rules.css")),
];

/// File name the syntect-generated highlight CSS (see
/// [`markdown::HIGHLIGHT_CSS`]) is written under. It is linked *after*
/// the static [`STYLESHEETS`] so its classes win where they overlap,
/// matching the cascade order of the previous single inline `<style>`.
pub(crate) const HIGHLIGHT_CSS_FILENAME: &str = "highlight.css";

/// The navigation script, written beside `index.html` and loaded via
/// `<script src>` rather than inlined.
pub(crate) const NAV_TOGGLE_SCRIPT: &str = include_str!("nav_toggle.js");

/// File name [`NAV_TOGGLE_SCRIPT`] is written under; the page's
/// `<script src>` references the same name, so they must agree.
pub(crate) const NAV_TOGGLE_SCRIPT_FILENAME: &str = "nav_toggle.js";

/// The chain-link glyph for the rule-name heading anchors, shipped as
/// a standalone file beside `index.html` rather than inlined.
pub(crate) const RULE_ANCHOR_ICON: &str = include_str!("assets/rule-anchor.svg");

/// File name [`RULE_ANCHOR_ICON`] is written under. `rules.css`
/// references the same name in a relative `url(...)`, so they must agree.
pub(crate) const RULE_ANCHOR_ICON_FILENAME: &str = "rule-anchor.svg";

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
                @for &(href, _) in STYLESHEETS {
                    link rel="stylesheet" href=(href);
                }
                link rel="stylesheet" href=(HIGHLIGHT_CSS_FILENAME);
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
                script src=(NAV_TOGGLE_SCRIPT_FILENAME) {}
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
/// The toggle is rendered with the HTML `hidden` attribute: the
/// click handler, scroll lock, focus moves, and `inert` setup are
/// all JS-driven, so a button visible to the reader without those
/// installed would be inert (a CSP-blocked or failed-to-load
/// external script, a stripped script tag, a parse error before the
/// handler attaches — any of these leave the page with a visible,
/// non-functional hamburger if the button isn't gated on script
/// readiness).
/// `<noscript>` only covers "scripting disabled in the browser",
/// not "script failed to run"; the `hidden` attribute covers both
/// uniformly. The script reveals the button by clearing `hidden`
/// once its handlers are wired up.
///
/// Below 1100px the nav becomes an overlay (a 280px panel on
/// phone-landscape / tablet, full-screen at <=600px / phone-
/// portrait). The JS also locks body scroll while it's open,
/// which stops the mobile URL bar from collapsing under it and
/// keeps every `position: fixed` element steady. The close (✕)
/// button lives inside the overlay in normal flow rather than as
/// a fixed-position sibling, so it's never affected by
/// visual-viewport quirks even on browsers where `position: fixed`
/// drifts with the URL bar.
fn nav_drawer(rules: &[Rule]) -> Markup {
    html! {
        button.nav-toggle
            type="button"
            hidden
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
                    a.rule-anchor href={ "#" (anchor_for(&rule.namespaced)) } aria-label="Permalink to this rule" {}
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
            (config_section(&rule.config))
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

/// Build the in-page anchor for a rule. The fragment is
/// `/rule/<kebab-name>` — a `/rule/` prefix plus the rule's
/// unnamespaced name with `_` swapped for `-` — so a permalink
/// reads `#/rule/qualified-paths` rather than the old
/// `#perfectionist-qualified_paths`. The leading slash makes the
/// fragment look like a route, and the value doubles as the
/// target element's `id`. Slashes are legal in an HTML `id`
/// (only ASCII whitespace is forbidden) and in a URL fragment,
/// but they are *not* legal in a bare CSS id selector, so any JS
/// that resolves the fragment must use `getElementById`, not
/// `querySelector("#" + ...)`.
fn anchor_for(namespaced: &str) -> String {
    format!("/rule/{}", unnamespaced(namespaced).replace('_', "-"))
}

fn unnamespaced(namespaced: &str) -> &str {
    namespaced.strip_prefix(NAMESPACE).unwrap_or(namespaced)
}

#[cfg(test)]
mod tests;
