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

use crate::model::{DefaultState, NAMESPACE, RenderContext, Rule};
use crate::render::config::config_section;
use crate::render::markdown::{markdown_inline_to_html, markdown_to_html};
use maud::{DOCTYPE, Markup, PreEscaped, html};

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
    ("settings.css", include_str!("style/settings.css")),
    ("light.css", include_str!("style/light.css")),
    ("dark.css", include_str!("style/dark.css")),
];

/// File name the light-mode syntect-generated highlight CSS (see
/// [`markdown::HIGHLIGHT_CSS`]`.light`) is written under. It is linked
/// *after* the static [`STYLESHEETS`] so its classes win where they
/// overlap, matching the cascade order of the previous single inline
/// `<style>`. Named `highlight-light.css` for symmetry with its dark
/// counterpart.
pub(crate) const HIGHLIGHT_CSS_LIGHT_FILENAME: &str = "highlight-light.css";

/// File name the dark-mode highlight CSS (see
/// [`markdown::HIGHLIGHT_CSS`]`.dark`) is written under. Linked after
/// [`HIGHLIGHT_CSS_LIGHT_FILENAME`]; its rules are scoped under a
/// higher-specificity ancestor so they re-colour the same syntax
/// classes only when the effective theme is dark.
pub(crate) const HIGHLIGHT_CSS_DARK_FILENAME: &str = "highlight-dark.css";

/// The navigation script, written beside `index.html` and loaded via
/// `<script src>` rather than inlined.
pub(crate) const NAV_TOGGLE_SCRIPT: &str = include_str!("nav_toggle.js");

/// File name [`NAV_TOGGLE_SCRIPT`] is written under; the page's
/// `<script src>` references the same name, so they must agree.
pub(crate) const NAV_TOGGLE_SCRIPT_FILENAME: &str = "nav_toggle.js";

/// The theme (colour-scheme) settings script, written beside
/// `index.html` and loaded via `<script src>` rather than inlined.
pub(crate) const THEME_TOGGLE_SCRIPT: &str = include_str!("theme_toggle.js");

/// File name [`THEME_TOGGLE_SCRIPT`] is written under; the page's
/// `<script src>` references the same name, so they must agree.
pub(crate) const THEME_TOGGLE_SCRIPT_FILENAME: &str = "theme_toggle.js";

/// The Configuration "Expand all" / "Collapse all" script, written
/// beside `index.html` and loaded via `<script src>` rather than
/// inlined. Kept separate from [`THEME_TOGGLE_SCRIPT`] so the two
/// Settings sections degrade independently: if one script fails to
/// run, its controls stay hidden while the other's keep working.
pub(crate) const CONFIG_TOGGLE_SCRIPT: &str = include_str!("config_toggle.js");

/// File name [`CONFIG_TOGGLE_SCRIPT`] is written under; the page's
/// `<script src>` references the same name, so they must agree.
pub(crate) const CONFIG_TOGGLE_SCRIPT_FILENAME: &str = "config_toggle.js";

/// The page scripts, in the order `<body>` loads them. Both the
/// foot-of-`<body>` `<script src>` tags and the `<head>`
/// `<link rel="preload" as="script">` hints are generated from this slice,
/// so they can't drift.
pub(crate) const PAGE_SCRIPTS: &[&str] = &[
    NAV_TOGGLE_SCRIPT_FILENAME,
    THEME_TOGGLE_SCRIPT_FILENAME,
    CONFIG_TOGGLE_SCRIPT_FILENAME,
];

/// The colour-scheme icons (Octicons, MIT), shipped beside `index.html`
/// and referenced from settings.css. Each tuple is `(filename, contents)`.
pub(crate) const THEME_ICONS: &[(&str, &str)] = &[
    ("theme-light.svg", include_str!("assets/theme-light.svg")),
    ("theme-dark.svg", include_str!("assets/theme-dark.svg")),
    ("theme-system.svg", include_str!("assets/theme-system.svg")),
];

/// `id` shared by the inert prefetch `<template>`
/// ([`theme_icon_prefetch_template`]) and the `theme_toggle.js` lookup that
/// activates it; the two must agree.
pub(crate) const THEME_ICON_PREFETCH_TEMPLATE_ID: &str = "theme-icon-prefetch";

/// The chain-link glyph for the rule-name heading anchors, shipped as
/// a standalone file beside `index.html` rather than inlined.
pub(crate) const RULE_ANCHOR_ICON: &str = include_str!("assets/rule-anchor.svg");

/// File name [`RULE_ANCHOR_ICON`] is written under. `rules.css`
/// references the same name in a relative `url(...)`, so they must agree.
pub(crate) const RULE_ANCHOR_ICON_FILENAME: &str = "rule-anchor.svg";

/// The body-text webfont (Cantarell, latin subset), shipped beside
/// `index.html` so the `@font-face` `url(...)` fallback in base.css
/// resolves without reaching an external CDN. Each tuple is
/// `(filename, bytes)`; the WOFF2 payloads are embedded with
/// `include_bytes!`. The matching `@font-face` rules list
/// `local("Cantarell")` first, so a reader whose system already has
/// Cantarell never downloads these — they only load as a fallback.
/// Cantarell is licensed under the SIL Open Font License 1.1; the
/// license text rides along as [`CANTARELL_LICENSE`].
pub(crate) const FONT_ASSETS: &[(&str, &[u8])] = &[
    (
        "cantarell-regular.woff2",
        include_bytes!("assets/cantarell-regular.woff2"),
    ),
    (
        "cantarell-bold.woff2",
        include_bytes!("assets/cantarell-bold.woff2"),
    ),
];

/// The SIL Open Font License 1.1 text for the bundled Cantarell
/// [`FONT_ASSETS`], shipped beside `index.html` to satisfy the
/// license's requirement that it accompany the redistributed font.
pub(crate) const CANTARELL_LICENSE: &str = include_str!("assets/Cantarell-OFL.txt");

/// File name [`CANTARELL_LICENSE`] is written under.
pub(crate) const CANTARELL_LICENSE_FILENAME: &str = "Cantarell-OFL.txt";

pub(crate) fn render_page(rules: &[Rule], context: &RenderContext<'_>) -> String {
    let RenderContext {
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
                    "perfectionist lints — " (git_ref)
                }
                @for &(href, _) in STYLESHEETS {
                    link rel="stylesheet" href=(href);
                }
                link rel="stylesheet" href=(HIGHLIGHT_CSS_LIGHT_FILENAME);
                link rel="stylesheet" href=(HIGHLIGHT_CSS_DARK_FILENAME);
                @for &src in PAGE_SCRIPTS {
                    link rel="preload" as="script" href=(src);
                }
            }
            body {
                h1 id="catalogue" { "perfectionist lints" }
                (nav_drawer(rules))
                (settings_panel())
                (theme_icon_prefetch_template())
                div.banner {
                    "Showing docs for " code { (git_ref) } "."
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
                @for &src in PAGE_SCRIPTS {
                    script src=(src) {}
                }
            }
        }
    };
    markup.into_string()
}

/// The colour-scheme settings affordance: a gear button fixed at the
/// top-right (mirroring the nav hamburger at the top-left) and the panel
/// it toggles. Both are rendered with the HTML `hidden` attribute and
/// revealed by `theme_toggle.js` only once its handlers are wired up, so
/// a page whose script never runs shows neither a dead gear nor an
/// orphan panel (the `[hidden]` reset in base.css makes that
/// unconditional). The panel holds two sections: "Theme" — three
/// radios (Light / Dark / System) styled as icon tiles, with System
/// the default — and "Configuration", the two bulk-toggle buttons
/// (see [`config_controls`]). It is shaped to gain further sections.
fn settings_panel() -> Markup {
    html! {
        button.settings-toggle
            type="button"
            hidden
            aria-controls="settings-panel"
            aria-expanded="false"
            aria-label="Settings"
            title="Settings" {}
        div.settings-panel id="settings-panel" hidden aria-label="Settings" {
            p.settings-panel-title { "Settings" }
            fieldset.settings-section {
                legend { "Theme" }
                div.theme-options {
                    (theme_option("light", "color-scheme-light", "Light", false))
                    (theme_option("dark", "color-scheme-dark", "Dark", false))
                    (theme_option("system", "color-scheme-system", "System", true))
                }
            }
            (config_controls())
        }
    }
}

/// The "Configuration" Settings section: a pair of stateless buttons
/// that open ("Expand all") or close ("Collapse all") every rule's
/// Configuration `<details>` (`details.config-details`) at once.
/// Toggling every `<details>` from one control has no pure-CSS
/// expression, so the buttons are driven by `config_toggle.js`.
///
/// The whole `<fieldset>` is emitted with the HTML `hidden` attribute
/// and revealed by that script only once its click handlers are wired
/// up — the same "reveal only when functional" contract the gear and
/// hamburger follow (the `[hidden]` reset in base.css makes it
/// unconditional). Hiding the section, rather than just the two
/// buttons, also keeps the "Configuration" legend from showing alone
/// above an empty row when the script never runs. The `data-config-open`
/// attribute selects each button; the script also reflects the page's
/// open/closed mix back onto them via `aria-pressed` (highlighting
/// "Expand all" when every panel is open, "Collapse all" when every
/// panel is closed), so they carry no `aria-pressed` until the script
/// turns them into toggle buttons.
fn config_controls() -> Markup {
    html! {
        fieldset.settings-section.config-controls hidden {
            legend { "Configuration" }
            div.config-actions {
                button.config-action type="button" data-config-open="true" { "Expand all" }
                button.config-action type="button" data-config-open="false" { "Collapse all" }
            }
        }
    }
}

/// The inert `<template>` of `<link rel="prefetch" as="image">` hints, one
/// per [`THEME_ICONS`] entry, warming the cache for the colour-scheme icons
/// (reachable only through settings.css masks). The `<template>` keeps the
/// links dormant until `theme_toggle.js` clones them into `<head>`, so they
/// load only once the Settings panel is used.
fn theme_icon_prefetch_template() -> Markup {
    html! {
        template id=(THEME_ICON_PREFETCH_TEMPLATE_ID) {
            @for &(name, _) in THEME_ICONS {
                link rel="prefetch" as="image" href=(name);
            }
        }
    }
}

/// One theme radio plus its visible label. The radio keeps real
/// `<input type="radio">` semantics (exclusive choice, keyboard arrows,
/// form labelling) but is visually hidden by settings.css; the adjacent
/// `<label>` is the styled tile, so the pure-CSS
/// `.theme-radio:checked + .theme-option` selector can highlight the
/// chosen one. The label must therefore stay the input's immediate next
/// sibling. The icon is an empty span the stylesheet fills via a CSS
/// mask referencing one of [`THEME_ICONS`].
fn theme_option(value: &str, id: &str, label: &str, checked: bool) -> Markup {
    let option_class = format!("theme-option theme-option-{value}");
    html! {
        input.theme-radio
            type="radio"
            name="color-scheme"
            id=(id)
            value=(value)
            checked[checked];
        label class=(option_class) for=(id) {
            span.theme-icon aria-hidden="true" {}
            span.theme-label { (label) }
        }
    }
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
