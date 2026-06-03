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
/// articles emit substrings (`href="#/rule/*"`,
/// `<code>name</code>`, `id="/rule/*"`) that overlap
/// the sidebar's markup.
fn sidebar_list(html: &str) -> &str {
    let open = r#"<ul class="nav-sidebar-list">"#;
    let close = "</ul>";
    let start = html.find(open).expect("sidebar <ul> not rendered") + open.len();
    let end = start
        + html[start..]
            .find(close)
            .expect("sidebar <ul> unterminated");
    &html[start..end]
}

#[test]
fn anchor_for_builds_a_kebab_cased_rule_route() {
    // The `/rule/` prefix plus a kebab-cased name: a single
    // word is unchanged, a multi-word name has every `_`
    // turned into `-`. The namespace prefix is dropped.
    assert_eq!(anchor_for("perfectionist::alpha"), "/rule/alpha");
    assert_eq!(
        anchor_for("perfectionist::beta_gamma_delta"),
        "/rule/beta-gamma-delta",
    );
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
    assert!(html.contains(r#"<button class="nav-toggle" type="button" hidden "#));
    assert!(html.contains(r#"aria-controls="nav-sidebar""#));
    assert!(html.contains(r#"aria-expanded="false""#));
    assert!(html.contains(r#"aria-label="Toggle navigation""#));
    assert!(html.contains(r#"aria-label="Lint rules""#));
    // The narrow-viewport CSS uses
    // `.nav-toggle[aria-expanded="true"] + .nav-sidebar`, so
    // the <nav> must be the immediate next sibling of
    // </button>. A refactor that nested the <nav> inside the
    // button or inserted another element between them would
    // break the overlay on narrow viewports; pin the literal
    // boundary.
    assert!(
        html.contains(r#"</button><nav class="nav-sidebar" id="nav-sidebar""#),
        r#"expected </button> to be immediately followed by the <nav class="nav-sidebar"> sibling"#,
    );
    // The close (✕) button lives inside the overlay so the
    // drawer can be dismissed without any fixed-position
    // element acting as both opener and closer. It must also
    // come *before* the title in DOM order: the hamburger
    // sits at the top-left, so on close the user's finger is
    // already there — putting the ✕ next to a tappable rule
    // link would invite a misclick on close.
    let header_start = html
        .find(r#"<div class="nav-sidebar-header">"#)
        .expect("nav-sidebar-header missing");
    let close_pos = html[header_start..]
        .find(r#"<button class="nav-sidebar-close""#)
        .expect("nav-sidebar-close missing");
    let title_pos = html[header_start..]
        .find(r#"<a class="nav-sidebar-title""#)
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
    assert!(sidebar.contains(r##"<li><a href="#/rule/alpha"><code>alpha</code></a></li>"##));
    // A multi-word rule name is rendered as-is (`beta_gamma`)
    // but its anchor is kebab-cased (`beta-gamma`).
    assert!(
        sidebar.contains(r##"<li><a href="#/rule/beta-gamma"><code>beta_gamma</code></a></li>"##),
    );
    // The rule articles those anchors resolve to must exist
    // outside the sidebar slice, otherwise the sidebar links
    // dangle.
    assert!(html.contains(r#"id="/rule/alpha""#));
    assert!(html.contains(r#"id="/rule/beta-gamma""#));
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
    // Both runtime-generated highlight sheets are linked — the page now
    // emits two, so a regression that drops the dark one must fail here.
    assert!(
        html.contains(&format!(
            "<link rel=\"stylesheet\" href=\"{HIGHLIGHT_CSS_LIGHT_FILENAME}\">"
        )),
        "expected a dedicated <link> for the light highlight CSS",
    );
    assert!(
        html.contains(&format!(
            "<link rel=\"stylesheet\" href=\"{HIGHLIGHT_CSS_DARK_FILENAME}\">"
        )),
        "expected a dedicated <link> for the dark highlight CSS",
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
            "<a class=\"rule-anchor\" href=\"#/rule/alpha\" \
             aria-label=\"Permalink to this rule\"></a>"
        ),
        "rule heading must emit a left-side permalink anchor to its own id",
    );
    let heading = r#"<h2><code><a class="rule-anchor""#;
    assert!(
        html.contains(heading),
        "the permalink anchor must be the first child of the rule heading's <code> (left side)",
    );
    // Tag-prefixed needles: the inlined stylesheet mentions these
    // as bare selectors, so a class-only search would match the CSS
    // in <head> before the body markup and scramble the ordering.
    let anchor_pos = html
        .find(r#"<a class="rule-anchor""#)
        .expect("rule-anchor missing");
    let code_pos = html
        .find(r#"<span class="lint-prefix""#)
        .expect("lint-prefix missing");
    let jump_pos = html
        .find(r#"<a class="rule-jump-link""#)
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
fn page_emits_settings_toggle_and_panel_both_hidden() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // The gear button mirrors the nav hamburger: a plain <button>
    // driven by `aria-expanded` and emitted with the HTML `hidden`
    // attribute so it only appears once theme_toggle.js wires up its
    // handlers and clears `hidden`. Anchor the assertion to the
    // toggle's opening tag so a refactor that drops `hidden` is caught.
    assert!(html.contains(r#"<button class="settings-toggle" type="button" hidden "#));
    assert!(html.contains(r#"aria-controls="settings-panel""#));
    assert!(html.contains(r#"aria-label="Settings""#));
    // The panel is also `hidden` by default — the script reveals only
    // the gear, and the gear's click handler toggles the panel.
    assert!(html.contains(r#"<div class="settings-panel" id="settings-panel" hidden "#));
}

#[test]
fn page_emits_three_theme_radios_with_system_default() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // Real radios (exclusive choice, keyboard arrows, form labelling)
    // grouped under one name. Three of them: light, dark, system.
    assert_eq!(
        html.matches(r#"<input class="theme-radio" type="radio" name="color-scheme""#)
            .count(),
        3,
    );
    for value in ["light", "dark", "system"] {
        assert!(
            html.contains(&format!(r#"value="{value}""#)),
            "expected a {value} theme radio",
        );
    }
    // System is the default choice (override unset), so only it is
    // rendered `checked`. The light/dark radios must not carry it.
    assert!(html.contains(r#"id="color-scheme-system" value="system" checked"#));
    assert!(!html.contains(r#"value="light" checked"#));
    assert!(!html.contains(r#"value="dark" checked"#));
    // Each radio is immediately followed by its <label> tile so the
    // pure-CSS `.theme-radio:checked + .theme-option` highlight works.
    assert!(
        html.contains(r#"value="system" checked><label class="theme-option theme-option-system""#),
    );
}

#[test]
fn page_links_theme_toggle_script_externally() {
    let html = render_page(&[fake_rule("only")], &fake_context());
    // The theme script ships as a sibling file loaded via `<script
    // src>`, not inlined — same contract as the nav script.
    assert!(
        html.contains(&format!(
            "<script src=\"{THEME_TOGGLE_SCRIPT_FILENAME}\"></script>"
        )),
        "expected the theme script to be referenced via <script src>",
    );
    assert!(
        !html.contains("<script>(function () {"),
        "the theme script must not be inlined into the page",
    );
}

#[test]
fn page_emits_config_controls_section_hidden_with_two_buttons() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // The Configuration section is a <fieldset> emitted with the HTML
    // `hidden` attribute: config_toggle.js reveals it only once its
    // click handlers are wired up, so a page whose script never runs
    // shows neither dead buttons nor an orphan "Configuration" legend.
    // Anchor to the opening tag so a refactor that drops `hidden`
    // (or hides only the buttons, leaving the legend) is caught.
    assert!(
        html.contains(r#"<fieldset class="settings-section config-controls" hidden>"#),
        "the Configuration section must be a `hidden` <fieldset>",
    );
    assert!(html.contains("<legend>Configuration</legend>"));
    // Two stateless buttons. `data-config-open` carries the action so
    // the script needs no per-button branching; pin both values.
    assert!(
        html.contains(
            r#"<button class="config-action" type="button" data-config-open="true">Expand all</button>"#
        ),
        "expected an `Expand all` button carrying data-config-open=true",
    );
    assert!(
        html.contains(
            r#"<button class="config-action" type="button" data-config-open="false">Collapse all</button>"#
        ),
        "expected a `Collapse all` button carrying data-config-open=false",
    );
}

#[test]
fn page_links_config_toggle_script_externally() {
    let html = render_page(&[fake_rule("only")], &fake_context());
    // The config-toggle script ships as a sibling file loaded via
    // `<script src>`, not inlined — same contract as the nav and theme
    // scripts.
    assert!(
        html.contains(&format!(
            "<script src=\"{CONFIG_TOGGLE_SCRIPT_FILENAME}\"></script>"
        )),
        "expected the config-toggle script to be referenced via <script src>",
    );
    assert!(
        !html.contains("<script>(function () {"),
        "the config-toggle script must not be inlined into the page",
    );
}

#[test]
fn config_toggle_script_reflects_state_onto_buttons() {
    // The buttons highlight when every panel shares one state. That state
    // is reflected onto `aria-pressed` (the CSS hook the colour layer keys
    // off) and kept live by listening for the <details> `toggle` event in
    // the capture phase (it doesn't bubble) and coalescing a burst of them
    // into one recompute per animation frame.
    assert!(
        CONFIG_TOGGLE_SCRIPT.contains("aria-pressed"),
        "the script must reflect state onto aria-pressed",
    );
    assert!(
        CONFIG_TOGGLE_SCRIPT.contains(r#""toggle""#),
        "the script must listen for the <details> toggle event",
    );
    assert!(
        CONFIG_TOGGLE_SCRIPT.contains("requestAnimationFrame"),
        "the script must coalesce toggle bursts via requestAnimationFrame",
    );
    // The highlight colours live in the colour layer (light.css / dark.css),
    // not the structural settings.css — same split as the theme tiles'
    // checked highlight. Both layers must carry the rule, or the highlight
    // silently vanishes in one theme; pin each so the dark variant can't be
    // dropped while the light one keeps the test green.
    assert!(stylesheet("light.css").contains(r#".config-action[aria-pressed="true"]"#));
    assert!(stylesheet("dark.css").contains(r#".config-action[aria-pressed="true"]"#));
}

#[test]
fn config_toggle_script_is_a_single_iife() {
    // Same structural sanity check as `nav_toggle_script_is_a_single_iife`:
    // the whole file is one IIFE so a stray block after the closer can't
    // reference a `var` that's already out of scope.
    assert_eq!(CONFIG_TOGGLE_SCRIPT.matches("(function () {").count(), 1);
    assert_eq!(CONFIG_TOGGLE_SCRIPT.matches("})();").count(), 1);
}

#[test]
fn settings_css_keeps_theme_radios_visually_hidden() {
    // The radios stay in the DOM (for semantics) but must be visually
    // removed; the visible control is the adjacent label. Pin the
    // visually-hidden recipe so a refactor can't make raw radio circles
    // reappear next to the styled tiles.
    let settings = stylesheet("settings.css");
    assert!(
        settings.contains(".theme-radio") && settings.contains("clip: rect(0, 0, 0, 0)"),
        "settings.css must keep the .theme-radio visually-hidden recipe",
    );
    // The checked radio highlights its sibling label via a pure-CSS
    // adjacent-sibling selector (no JS needed for the highlight). The
    // highlight is a colour, so it lives in the colour layer (light.css),
    // not the structural settings.css.
    assert!(stylesheet("light.css").contains(".theme-radio:checked + .theme-option"));
}

#[test]
fn page_does_not_inline_theme_icon_svgs() {
    // The three colour-scheme icons stay external (referenced as CSS
    // masks in settings.css), never inlined — same rule as the anchor
    // icon. The `page_does_not_inline_the_anchor_icon_svg` test already
    // forbids any `<svg`, but assert the masks reference the files so a
    // rename can't silently 404 them.
    let settings = stylesheet("settings.css");
    for (name, _) in THEME_ICONS {
        assert!(
            settings.contains(&format!("url(\"{name}\")")),
            "settings.css must reference the theme icon {name} as a mask",
        );
    }
}

#[test]
fn theme_files_carry_literal_colours_not_variables() {
    // The colour layers use literal colours, never CSS custom
    // properties, so the page themes on engines that predate `var()`.
    // light.css holds the default light values; dark.css holds the dark
    // values under the two override-tier prefixes.
    // Strip comments first: the files' header comments mention `var()`
    // while explaining why they avoid it.
    let light = strip_css_comments(stylesheet("light.css"));
    assert!(
        !light.contains("var(") && !light.contains("--color"),
        "light.css must not use CSS custom properties",
    );
    assert!(light.contains("background-color: #ffffff"));

    let dark = strip_css_comments(stylesheet("dark.css"));
    assert!(
        !dark.contains("var(") && !dark.contains("--color"),
        "dark.css must not use CSS custom properties",
    );
    // Tier 2 (system preference) and tier 3 (explicit Dark) prefixes,
    // keyed off the <html> attribute theme_toggle.js sets.
    assert!(dark.contains("@media (prefers-color-scheme: dark)"));
    assert!(dark.contains(r#"html:not([color-scheme-override="light"])"#));
    assert!(dark.contains(r#"html[color-scheme-override="dark"]"#));
    assert!(dark.contains("background-color: #0d1117"));
}

/// Strip `/* ... */` block comments (the only comment form CSS has) so
/// colour scans don't trip over prose or issue references inside them.
fn strip_css_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start..].find("*/") {
            Some(end) => rest = &rest[start + end + "*/".len()..],
            None => return out, // unterminated; ignore the tail
        }
    }
    out.push_str(rest);
    out
}

#[test]
fn structural_sheets_carry_no_literal_colours() {
    // The colours were extracted out of the structural sheets; they
    // should hold layout/sizing only, so a colour change is a one-file
    // edit in the colour layer. `currentColor` (a structural reference,
    // not a theme value) and `#id` selectors like `#catalogue` are
    // fine — only literal `#rrggbb` hexes and `rgb(`/`rgba(` are colours.
    for name in ["base.css", "nav.css", "rules.css", "settings.css"] {
        // Strip comments first: prose may mention colours or `var()`.
        let code = strip_css_comments(stylesheet(name));
        assert!(
            !code.contains("var("),
            "{name} must not reference CSS custom properties",
        );
        assert!(
            !code.contains("rgb"),
            "{name} should carry no literal rgb()/rgba() colour",
        );
        // A `#` followed by six hex digits is a colour literal; `#id`
        // selectors fail the six-hex test (they contain non-hex letters).
        let bytes = code.as_bytes();
        for (index, &byte) in bytes.iter().enumerate() {
            if byte != b'#' {
                continue;
            }
            let is_six_hex = bytes
                .get(index + 1..index + 7)
                .is_some_and(|run| run.iter().all(|byte| byte.is_ascii_hexdigit()));
            assert!(
                !is_six_hex,
                "{name} should carry no literal hex colour near byte {index}",
            );
        }
    }
}

#[test]
fn dark_highlight_css_is_scoped_to_the_dark_scheme() {
    let dark = &crate::render::markdown::HIGHLIGHT_CSS.dark;
    // The dark syntax sheet re-styles the same classes the light sheet
    // emits, so it must be scoped under a higher-specificity ancestor
    // for both the system-preference and explicit-override tiers, or it
    // would unconditionally clobber the light colours.
    assert!(dark.contains("@media (prefers-color-scheme: dark)"));
    assert!(
        dark.contains(r#"html:not([color-scheme-override="light"]) .comment"#),
        "system-preference dark highlight must scope the syntax classes",
    );
    assert!(
        dark.contains(r#"html[color-scheme-override="dark"] .comment"#),
        "explicit-Dark highlight must scope the syntax classes",
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
