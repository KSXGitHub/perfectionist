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
    use crate::model::ConfigDoc;

    fn fake_rule(name: &str) -> Rule {
        Rule {
            namespaced: format!("perfectionist::{name}"),
            default_state: DefaultState::Active,
            short_desc: format!("demo rule {name}"),
            doc_markdown: "### What it does\nDoes a demo.".to_owned(),
            relative_source: PathBuf::from(format!("src/rules/{name}.rs")),
            config: ConfigDoc {
                key: format!("perfectionist::{name}"),
                fields: Vec::new(),
                custom_types: Vec::new(),
            },
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
        // attribute. The button starts in the closed state and is
        // emitted with `hidden` so it appears only after the script
        // wires up its (entirely JS-driven) behaviour — otherwise a
        // CSP-blocked or otherwise non-executing script would leave
        // a visible-but-inert hamburger on the page.
        // The button must literally carry the `hidden` attribute —
        // not just have the word "hidden" somewhere on the page
        // (the CSS comments mention `[hidden]` repeatedly). Anchor
        // the assertion to the toggle's opening tag so a refactor
        // that drops `hidden` from the template can't slip past.
        assert!(html.contains("<button class=\"nav-toggle\" type=\"button\" hidden "));
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
    fn page_links_nav_toggle_script_externally() {
        let html = render_page(&[fake_rule("only")], &fake_context());
        // The script ships as a sibling file and is loaded via
        // `<script src>`, not inlined. Pin the exact tag so a
        // refactor that re-inlines the body (or renames the file
        // out of sync with `NAV_TOGGLE_SCRIPT_FILENAME`) is caught.
        assert!(
            html.contains(&format!(
                "<script src=\"{NAV_TOGGLE_SCRIPT_FILENAME}\"></script>"
            )),
            "expected the nav script to be referenced via <script src>",
        );
        // And the body must not be inlined any more: a `<script>`
        // with content would carry the IIFE opener verbatim.
        assert!(
            !html.contains("<script>(function () {"),
            "the nav script must not be inlined into the page",
        );
        // The page no longer ships a `<noscript>` rule: the toggle
        // is rendered with the HTML `hidden` attribute and the
        // script clears it once it wires up handlers, which covers
        // every "JS isn't running" mode — `<noscript>` only
        // covers "scripting disabled in browser" and isn't needed.
        // The JS file's own comments mention the term "noscript",
        // so we check for the closing tag (which only appears in
        // the actual element, never in a free-text comment).
        assert!(!html.contains("</noscript>"));
    }

    #[test]
    fn page_links_each_stylesheet_individually() {
        let html = render_page(&[fake_rule("alpha")], &fake_context());
        // Every static sheet gets its own `<link>`, in slice order,
        // and the runtime-generated highlight sheet is linked last so
        // its cascade position matches the old single `<style>`.
        for &(name, _) in STYLESHEETS {
            assert!(
                html.contains(&format!("<link rel=\"stylesheet\" href=\"{name}\">")),
                "expected a dedicated <link> for {name}",
            );
        }
        assert!(
            html.contains(&format!(
                "<link rel=\"stylesheet\" href=\"{HIGHLIGHT_CSS_FILENAME}\">"
            )),
            "expected a dedicated <link> for the highlight CSS",
        );
        // Nothing is inlined any more: no `<style>` block survives.
        assert!(
            !html.contains("<style"),
            "stylesheets must be external; no inline <style> should remain",
        );
    }

    /// Look a stylesheet up by its emitted file name. Tests assert
    /// against the specific sheet that owns a declaration rather
    /// than the whole bundle, mirroring the one-file-per-sheet
    /// layout the page now links.
    fn stylesheet(name: &str) -> &'static str {
        STYLESHEETS
            .iter()
            .find(|(sheet_name, _)| *sheet_name == name)
            .map(|(_, content)| *content)
            .unwrap_or_else(|| panic!("no stylesheet named {name}"))
    }

    #[test]
    fn style_sheet_contains_hidden_attribute_reset() {
        // The `hidden`-attribute + reveal-on-ready design only
        // works if author CSS doesn't silently override the UA's
        // `[hidden] { display: none }` rule. The `.nav-toggle`
        // selector sets `display: flex` at equal specificity, so
        // an author-side `[hidden] { display: none !important }`
        // reset is load-bearing — without it the toggle would be
        // visible despite the Rust template emitting `<button
        // hidden ...>`, defeating the whole "JS isn't running"
        // fallback. Pin the reset's presence in base.css so a
        // future edit can't quietly remove it.
        let base = stylesheet("base.css");
        assert!(
            base.contains("[hidden]") && base.contains("display: none !important"),
            "style/base.css must keep the `[hidden] {{ display: none !important }}` reset; \
             without it the JS-driven toggle's `hidden`-by-default fallback is broken",
        );
    }

    #[test]
    fn rule_heading_emits_left_side_permalink_anchor() {
        let html = render_page(&[fake_rule("alpha")], &fake_context());
        assert!(
            html.contains(
                "<a class=\"rule-anchor\" href=\"#perfectionist-alpha\" \
                 aria-label=\"Permalink to this rule\"></a>"
            ),
            "rule heading must emit a left-side permalink anchor to its own id",
        );
        let heading = "<h2><code><a class=\"rule-anchor\"";
        assert!(
            html.contains(heading),
            "the permalink anchor must be the first child of the rule heading's <code> (left side)",
        );
        // Tag-prefixed needles: the inlined stylesheet mentions these
        // as bare selectors, so a class-only search would match the CSS
        // in <head> before the body markup and scramble the ordering.
        let anchor_pos = html
            .find("<a class=\"rule-anchor\"")
            .expect("rule-anchor missing");
        let code_pos = html
            .find("<span class=\"lint-prefix\"")
            .expect("lint-prefix missing");
        let jump_pos = html
            .find("<a class=\"rule-jump-link\"")
            .expect("jump link missing");
        assert!(
            anchor_pos < code_pos && code_pos < jump_pos,
            "permalink anchor must precede the rule name, which precedes the jump link",
        );
    }

    #[test]
    fn page_does_not_inline_the_anchor_icon_svg() {
        let html = render_page(&[fake_rule("alpha")], &fake_context());
        assert!(
            !html.contains("<svg"),
            "the heading-anchor icon must stay an external resource, not inlined SVG",
        );
    }

    #[test]
    fn style_references_rule_anchor_icon() {
        // The CSS `url(...)` and the written filename must agree, or the
        // icon 404s.
        let expected = format!("url(\"{RULE_ANCHOR_ICON_FILENAME}\")");
        assert!(
            stylesheet("rules.css").contains(&expected),
            "rules.css must reference the anchor icon as {expected}",
        );
        assert!(
            RULE_ANCHOR_ICON.contains("Octicons") && RULE_ANCHOR_ICON.contains("MIT"),
            "the bundled rule-anchor.svg must retain its Octicons MIT attribution",
        );
    }

    #[test]
    fn nav_toggle_script_is_a_single_iife() {
        // Structural sanity check on `nav_toggle.js`. The whole
        // file is a single IIFE — every helper, every event
        // handler, every `var` is inside it. A duplicate-body bug
        // slipped past an earlier edit (commit f87b81d) by leaving
        // a stray `var ...; toggle.addEventListener(...); })();`
        // block AFTER the IIFE's closing line, producing a runtime
        // `ReferenceError: toggle is not defined` because those
        // `var`s were function-scoped to the now-closed IIFE.
        // Counting the IIFE opener and closer pins this shape so
        // a future edit can't reintroduce the same bug silently.
        assert_eq!(NAV_TOGGLE_SCRIPT.matches("(function () {").count(), 1);
        assert_eq!(NAV_TOGGLE_SCRIPT.matches("})();").count(), 1);
    }
}
