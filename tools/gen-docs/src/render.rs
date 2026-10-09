//! Render the collected [`Rule`]s into `index.html` plus the sibling
//! assets it links — the stylesheets ([`STYLESHEETS`]) and the page
//! scripts ([`PAGE_SCRIPTS`]), each written as a standalone file. The
//! page itself stays a single document so every reader of the catalogue
//! can ctrl-F across every rule's prose without page loads; the CSS and
//! JS live in their own files (rather than inlined) so each can be
//! edited and cached independently and so future changes can add or
//! split sheets without reflowing one monolith. GitHub Pages serves the
//! whole output directory verbatim.

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
    ("search.css", include_str!("style/search.css")),
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

/// Text handling for query matching: folding, splitting into words, and
/// stemming. The base of the matching stack — it reads none of the rest.
pub(crate) const MATCH_TEXT_SCRIPT: &str = include_str!("match_text.js");

/// File name [`MATCH_TEXT_SCRIPT`] is written under.
pub(crate) const MATCH_TEXT_SCRIPT_FILENAME: &str = "match_text.js";

/// What a match earns: every number the scoring uses, and the arithmetic
/// over them.
pub(crate) const MATCH_SCORE_SCRIPT: &str = include_str!("match_score.js");

/// File name [`MATCH_SCORE_SCRIPT`] is written under.
pub(crate) const MATCH_SCORE_SCRIPT_FILENAME: &str = "match_score.js";

/// Whether a match is worth showing, decided by where it landed and
/// reading no score.
pub(crate) const MATCH_ADMIT_SCRIPT: &str = include_str!("match_admit.js");

/// File name [`MATCH_ADMIT_SCRIPT`] is written under.
pub(crate) const MATCH_ADMIT_SCRIPT_FILENAME: &str = "match_admit.js";

/// The tiers a query is looked for in, from the query exactly through to
/// its characters scattered.
pub(crate) const MATCH_TIERS_SCRIPT: &str = include_str!("match_tiers.js");

/// File name [`MATCH_TIERS_SCRIPT`] is written under.
pub(crate) const MATCH_TIERS_SCRIPT_FILENAME: &str = "match_tiers.js";

/// The query-matching library the filter boxes and the search overlay
/// share, composed from the four above. It decides which targets a query
/// matches and how well, and nothing else: it draws no element and binds
/// no handler.
pub(crate) const MATCH_SCRIPT: &str = include_str!("match.js");

/// File name [`MATCH_SCRIPT`] is written under.
pub(crate) const MATCH_SCRIPT_FILENAME: &str = "match.js";

/// The ranking library: given the catalogue's rules as plain objects and
/// a query, it returns the handful that match, best first.
pub(crate) const RANK_SCRIPT: &str = include_str!("rank.js");

/// File name [`RANK_SCRIPT`] is written under.
pub(crate) const RANK_SCRIPT_FILENAME: &str = "rank.js";

/// The highlight library: it rebuilds an element's contents with each
/// matched range wrapped in a `<mark>`.
pub(crate) const HIGHLIGHT_SCRIPT: &str = include_str!("highlight.js");

/// File name [`HIGHLIGHT_SCRIPT`] is written under.
pub(crate) const HIGHLIGHT_SCRIPT_FILENAME: &str = "highlight.js";

/// The filter-box script: it clones each filter box from the
/// `<template>` [`filter_container`] emits, and narrows the list the box
/// sits over.
pub(crate) const FILTER_BOXES_SCRIPT: &str = include_str!("filter_boxes.js");

/// File name [`FILTER_BOXES_SCRIPT`] is written under.
pub(crate) const FILTER_BOXES_SCRIPT_FILENAME: &str = "filter_boxes.js";

/// The search-overlay script: it clones the overlay from the
/// `<template>`s [`search_templates`] emits and ranks the rules it reads
/// off the page. Kept
/// separate from [`FILTER_BOXES_SCRIPT`] so the two affordances degrade
/// independently: if one script fails to run, its controls stay hidden
/// while the other's keep working.
pub(crate) const SEARCH_OVERLAY_SCRIPT: &str = include_str!("search_overlay.js");

/// File name [`SEARCH_OVERLAY_SCRIPT`] is written under.
pub(crate) const SEARCH_OVERLAY_SCRIPT_FILENAME: &str = "search_overlay.js";

/// The page scripts, in the order `<body>` loads them. Both the
/// foot-of-`<body>` `<script src>` tags and the `<head>`
/// `<link rel="preload" as="script">` hints are generated from this slice,
/// so they can't drift.
///
/// Every library precedes whatever reads its global: the four files
/// `match.js` is composed from precede it, and it precedes `rank.js`,
/// which reads its matchers.
/// Every page script with the source it ships, as `(filename, contents)`.
/// [`PAGE_SCRIPTS`] is this list's names, so what the page loads is what
/// is written beside it.
pub(crate) const PAGE_SCRIPT_FILES: &[(&str, &str)] = &[
    (NAV_TOGGLE_SCRIPT_FILENAME, NAV_TOGGLE_SCRIPT),
    (THEME_TOGGLE_SCRIPT_FILENAME, THEME_TOGGLE_SCRIPT),
    (CONFIG_TOGGLE_SCRIPT_FILENAME, CONFIG_TOGGLE_SCRIPT),
    (MATCH_TEXT_SCRIPT_FILENAME, MATCH_TEXT_SCRIPT),
    (MATCH_SCORE_SCRIPT_FILENAME, MATCH_SCORE_SCRIPT),
    (MATCH_ADMIT_SCRIPT_FILENAME, MATCH_ADMIT_SCRIPT),
    (MATCH_TIERS_SCRIPT_FILENAME, MATCH_TIERS_SCRIPT),
    (MATCH_SCRIPT_FILENAME, MATCH_SCRIPT),
    (RANK_SCRIPT_FILENAME, RANK_SCRIPT),
    (HIGHLIGHT_SCRIPT_FILENAME, HIGHLIGHT_SCRIPT),
    (FILTER_BOXES_SCRIPT_FILENAME, FILTER_BOXES_SCRIPT),
    (SEARCH_OVERLAY_SCRIPT_FILENAME, SEARCH_OVERLAY_SCRIPT),
];

pub(crate) const PAGE_SCRIPTS: &[&str] = &[
    NAV_TOGGLE_SCRIPT_FILENAME,
    THEME_TOGGLE_SCRIPT_FILENAME,
    CONFIG_TOGGLE_SCRIPT_FILENAME,
    MATCH_TEXT_SCRIPT_FILENAME,
    MATCH_SCORE_SCRIPT_FILENAME,
    MATCH_ADMIT_SCRIPT_FILENAME,
    MATCH_TIERS_SCRIPT_FILENAME,
    MATCH_SCRIPT_FILENAME,
    RANK_SCRIPT_FILENAME,
    HIGHLIGHT_SCRIPT_FILENAME,
    FILTER_BOXES_SCRIPT_FILENAME,
    SEARCH_OVERLAY_SCRIPT_FILENAME,
];

/// `id` of the magnifier symbol, on the button that opens the search
/// overlay.
const SEARCH_ICON_ID: &str = "icon-search";

/// `id` of the funnel symbol, on both filter toggles.
const FILTER_ICON_ID: &str = "icon-filter";

/// `id` of the chain-link symbol, on each rule heading's permalink.
const RULE_ANCHOR_ICON_ID: &str = "icon-rule-anchor";

/// `id` of the search overlay itself, shared by the overlay markup in
/// [`search_templates`] and the [`search_toggle`] button's
/// `aria-controls`.
pub(crate) const SEARCH_OVERLAY_ID: &str = "search-overlay";

/// `id` shared by the inert `<template>` holding the search overlay's
/// markup ([`search_templates`]) and the `search_overlay.js` lookup that
/// clones it into the page.
pub(crate) const SEARCH_OVERLAY_TEMPLATE_ID: &str = "search-overlay-template";

/// `id` shared by the inert `<template>` holding one search result's
/// markup ([`search_templates`]) and the `search_overlay.js` lookup that
/// clones it per result.
pub(crate) const SEARCH_RESULT_TEMPLATE_ID: &str = "search-result-template";

/// Accessible name of the Index filter, carried by both its funnel
/// button and the input the button opens. One phrase for both, so the
/// control and the box it opens announce as the same thing.
const INDEX_FILTER_LABEL: &str = "Filter the index by lint name";

/// Accessible name of the navigation filter. See [`INDEX_FILTER_LABEL`].
const NAV_FILTER_LABEL: &str = "Filter the navigation by lint name";

/// The icon definitions ([`crate::icons::sprite`]), emitted once at the
/// foot of `<body>`, after the content and before [`PAGE_SCRIPTS`].
///
/// Every [`icon`] therefore refers forward to a symbol the parser has
/// not reached yet, which renders nothing until it does and then
/// resolves on its own. Nothing on this page observes that window: the
/// search, filter and theme icons sit on controls emitted `hidden`,
/// which only their scripts reveal, and those scripts load from below
/// this point; the rule-heading anchors are the one exception and are
/// transparent until their heading is hovered.
///
/// The sprite carries the HTML `hidden` attribute, which base.css makes
/// unconditional with `[hidden] { display: none !important }`. A
/// `<symbol>` is not rendered where it is defined in any case; `hidden`
/// also keeps the element from occupying a line box of its own. Note
/// that this works *because* the icons are `<symbol>`s: `<use>` cannot
/// instantiate a `display: none` target, so a sprite of plain `<svg>`
/// elements hidden the same way would draw nothing at all.
fn icon_sprite() -> Markup {
    PreEscaped(crate::icons::sprite())
}

/// One icon, drawn by referring to the [`ICON_SPRITE`] symbol named by
/// `id`. `class` is what the stylesheets size and colour it through.
///
/// The reference is spelled twice. SVG 2's plain `href` is what every
/// current engine reads, but Safari did not accept it until 12.1 (iOS
/// 12.2); SVG 1.1's `xlink:href` is deprecated and read by everything
/// ever shipped. An engine that understands both prefers `href`. The
/// XLink namespace needs no `xmlns:xlink` declaration here, because the
/// HTML parser assigns it to that attribute itself.
///
/// Always `aria-hidden`: every icon in this page sits inside a control
/// that carries its own `aria-label`, so exposing the graphic as well
/// would name it twice.
fn icon(id: &str, class: &str) -> Markup {
    let href = format!("#{id}");
    html! {
        svg class=(class) aria-hidden="true" {
            use href=(href) xlink:href=(href) {}
        }
    }
}

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
                (search_toggle())
                (search_templates())
                (settings_panel())
                div.banner {
                    "Showing docs for " code { (git_ref) } "."
                }
                p {
                    "perfectionist is a Dylint plugin; see the "
                    a href=(repo_url) { "README" }
                    " for setup. Lint-control attributes use the "
                    code { (NAMESPACE) } " namespace."
                }
                h2.index-heading {
                    "Index"
                    (filter_toggle("index", INDEX_FILTER_LABEL))
                }
                (filter_container("index", INDEX_FILTER_LABEL))
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
                                        code { (breakable_lint_name(unnamespaced(&rule.namespaced))) }
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
                (icon_sprite())
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

/// The Search affordance's button, which opens the search overlay (see
/// [`search_templates`]).
///
/// It carries the HTML `hidden` attribute, cleared by
/// `search_overlay.js` once the overlay is in the page and its handlers
/// are wired up, so a page whose script never runs shows no dead button.
/// That is also the window in which its `aria-controls` names an element
/// not yet in the document: the button is `hidden` throughout it, so
/// nothing — assistive tech included — can follow the reference before
/// the overlay exists.
///
/// `search_overlay.js` also binds `/` to the same toggle, so the `title`
/// names the key while the `aria-label` stays the bare name: the label is
/// the button's accessible name, read on every visit, and a keyboard hint
/// in it would be read along with it.
fn search_toggle() -> Markup {
    html! {
        button.search-toggle
            type="button"
            hidden
            aria-controls=(SEARCH_OVERLAY_ID)
            aria-expanded="false"
            aria-label="Search lints"
            title="Search lints (press /)" {
            (icon(SEARCH_ICON_ID, "search-toggle-icon"))
        }
    }
}

/// The search overlay's markup, and one search result's, each inside an
/// inert `<template>`. Nothing inside one renders, takes focus or
/// answers `querySelector`, so a page whose script never runs has no
/// overlay in any sense that counts, while the markup stays here, where
/// it is reviewed alongside the rest of the page and tested without a
/// browser. `search_overlay.js` clones these and wires up the behaviour;
/// it builds no elements of its own.
///
/// The result template is a separate `<template>` rather than one nested
/// in the overlay's results list, because that list is emptied on every
/// keystroke and would take its own blueprint with it. Its state here is
/// the one the overlay opens in: nothing typed, so the prompt.
fn search_templates() -> Markup {
    html! {
        template id=(SEARCH_OVERLAY_TEMPLATE_ID) {
            div.search-overlay id=(SEARCH_OVERLAY_ID) hidden {
                div.search-dialog
                    role="dialog"
                    aria-modal="true"
                    aria-label="Search lints" {
                    div.search-box {
                        (search_text_input("search-input", "Search lints\u{2026}", "Search lints"))
                    }
                    // Hidden until there is something to list: an
                    // empty `<ul>` still claims the dialog's whole
                    // remaining height, which would push either message
                    // below it.
                    ul.search-results aria-label="Search results" hidden {}
                    // What the reader sees instead, one at a time. Both
                    // are rendered here rather than written by the script
                    // so the wording lives with the rest of the page's
                    // text. `role="status"` asks assistive tech to read
                    // whichever is revealed; support for that on an
                    // un-hidden region varies, so it is a courtesy, not
                    // the only way to learn the result.
                    p.search-empty.search-empty-prompt role="status" {
                        "Type to search lint names and documentation."
                    }
                    p.search-empty.search-empty-no-match role="status" hidden {
                        "No lint matches that search."
                    }
                    // Last, so it sits in the results' corner rather than
                    // in the search box, and so Tab reaches it after the
                    // results rather than before them.
                    button.search-close type="button" {
                        // The ✕ is decoration beside the word; hiding it
                        // from assistive tech keeps the button's
                        // accessible name the word alone.
                        span.search-close-glyph aria-hidden="true" { "\u{2715}" }
                        "Close"
                    }
                }
            }
        }
        template id=(SEARCH_RESULT_TEMPLATE_ID) {
            li {
                // No `href` until a result fills it in: the blueprint
                // points nowhere.
                a.search-result {
                    code.search-result-name {}
                    span.search-result-text {}
                }
            }
        }
    }
}

/// One query input, as both the search overlay and the filter boxes want
/// it.
///
/// The attributes after `type="search"` keep a phone or tablet keyboard
/// from capitalising the first letter and autocorrecting a half-typed
/// lint name into a dictionary word, neither of which can match a
/// snake_case identifier.
fn search_text_input(class: &str, placeholder: &str, label: &str) -> Markup {
    html! {
        input class=(class)
            type="search"
            placeholder=(placeholder)
            aria-label=(label)
            autocapitalize="none"
            autocorrect="off"
            autocomplete="off"
            enterkeyhint="search"
            spellcheck="false";
    }
}

/// The funnel button that shows and hides one filter box, right-aligned
/// on the same line as the heading it belongs to.
///
/// `kind` ties one filter box's names together: the toggle's
/// `<kind>-filter-toggle` class and the container's
/// `<kind>-filter-container` class are what `filter_boxes.js` selects
/// on, and `<kind>-filter` is the container's `id`, which is what this
/// button's `aria-controls` names. The page's are `index` and `nav`.
/// `label` is the whole affordance's accessible name, carried by this
/// button and by the input it opens alike.
///
/// Emitted `hidden` and revealed by that script only once the box it
/// opens has been built.
fn filter_toggle(kind: &str, label: &str) -> Markup {
    let class = format!("filter-toggle {kind}-filter-toggle");
    let controls = format!("{kind}-filter");
    html! {
        button class=(class)
            type="button"
            hidden
            aria-controls=(controls)
            aria-expanded="false"
            aria-label=(label)
            title=(label) {
            (icon(FILTER_ICON_ID, "filter-toggle-icon"))
        }
    }
}

/// Where one filter box goes, holding the inert `<template>` that is
/// its blueprint.
///
/// `filter_boxes.js` clones that template into this container and wires
/// it up; until it does, the container renders nothing and search.css
/// gives it no box of its own, so a page whose script never runs lays out
/// exactly as it would without the feature. The box inside the template
/// is itself `hidden`, so the clone starts closed and the funnel opens
/// it. See [`filter_toggle`] for what `kind` and `label` tie together.
fn filter_container(kind: &str, label: &str) -> Markup {
    let class = format!("filter-container {kind}-filter-container");
    let id = format!("{kind}-filter");
    html! {
        div class=(class) id=(id) {
            template {
                div.filter-box hidden {
                    (search_text_input("filter-input", "Filter by name\u{2026}", label))
                }
            }
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

/// One theme radio plus its visible label. The radio keeps real
/// `<input type="radio">` semantics (exclusive choice, keyboard arrows,
/// form labelling) but is visually hidden by settings.css; the adjacent
/// `<label>` is the styled tile, so the pure-CSS
/// `.theme-radio:checked + .theme-option` selector can highlight the
/// chosen one. The label must therefore stay the input's immediate next
/// sibling. The tile's icon is the sprite symbol named after `value`.
fn theme_option(value: &str, id: &str, label: &str, checked: bool) -> Markup {
    let option_class = format!("theme-option theme-option-{value}");
    let icon_id = format!("icon-theme-{value}");
    html! {
        input.theme-radio
            type="radio"
            name="color-scheme"
            id=(id)
            value=(value)
            checked[checked];
        label class=(option_class) for=(id) {
            (icon(&icon_id, "theme-icon"))
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
                (filter_toggle("nav", NAV_FILTER_LABEL))
            }
            (filter_container("nav", NAV_FILTER_LABEL))
            ul.nav-sidebar-list {
                @for rule in rules {
                    li {
                        a href={ "#" (anchor_for(&rule.namespaced)) } {
                            code { (breakable_lint_name(unnamespaced(&rule.namespaced))) }
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
                    a.rule-anchor href={ "#" (anchor_for(&rule.namespaced)) } aria-label="Permalink to this rule" {
                        (icon(RULE_ANCHOR_ICON_ID, "rule-anchor-icon"))
                    }
                    span.lint-prefix { (NAMESPACE) }
                    wbr;
                    span.lint-name { (breakable_lint_name(unnamespaced(&rule.namespaced))) }
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
/// reads `#/rule/path-qualification-mismatch` rather than the old
/// `#perfectionist-path_qualification_mismatch`. The leading slash makes the
/// fragment look like a route, and the value doubles as the
/// target element's `id`. Slashes are legal in an HTML `id`
/// (only ASCII whitespace is forbidden) and in a URL fragment,
/// but they are *not* legal in a bare CSS id selector, so any JS
/// that resolves the fragment must use `getElementById`, not
/// `querySelector("#" + ...)`.
fn anchor_for(namespaced: &str) -> String {
    format!("/rule/{}", unnamespaced(namespaced).replace('_', "-"))
}

/// Render a lint identifier with an explicit line-break opportunity
/// after each `_`.
fn breakable_lint_name(name: &str) -> Markup {
    html! {
        @for (index, segment) in name.split_inclusive('_').enumerate() {
            @if index > 0 {
                wbr;
            }
            (segment)
        }
    }
}

fn unnamespaced(namespaced: &str) -> &str {
    namespaced.strip_prefix(NAMESPACE).unwrap_or(namespaced)
}

#[cfg(test)]
mod test_assets;
#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod test_nav;
#[cfg(test)]
mod test_rules;
