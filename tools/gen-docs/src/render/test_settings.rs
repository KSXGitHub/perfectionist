//! The settings affordance: the gear, the panel it opens, the theme
//! radios and their icons, and the Configuration section's two bulk
//! buttons — with the scripts that drive each half, which degrade
//! independently and so are tested apart.

use super::test_fixtures::{fake_context, fake_rule, stylesheet};
use super::{
    CONFIG_TOGGLE_SCRIPT, CONFIG_TOGGLE_SCRIPT_FILENAME, THEME_ICON_PREFETCH_TEMPLATE_ID,
    THEME_ICONS, THEME_TOGGLE_SCRIPT, THEME_TOGGLE_SCRIPT_FILENAME, render_page,
};

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
            r#"<script src="{THEME_TOGGLE_SCRIPT_FILENAME}"></script>"#
        )),
        "expected the theme script to be referenced via <script src>",
    );
    assert!(
        !html.contains("<script>(function () {"),
        "the theme script must not be inlined into the page",
    );
}

#[test]
fn page_ships_theme_icon_prefetch_hints_in_an_inert_template() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // The colour-scheme icons are reachable only through settings.css masks,
    // invisible to the preload scanner. Instead of hardcoding their names in
    // JS, the page renders the prefetch hints from THEME_ICONS into an inert
    // <template>, so the warmed URLs are the same files settings.css masks —
    // testable here against the real output, not a string-match on the script.
    let open = format!(r#"<template id="{THEME_ICON_PREFETCH_TEMPLATE_ID}">"#);
    let template_start = html.find(&open).expect("prefetch <template> not rendered");
    let template_end = template_start
        + html[template_start..]
            .find("</template>")
            .expect("<template> unterminated")
        + "</template>".len();
    let template = &html[template_start..template_end];
    for (name, _) in THEME_ICONS {
        assert!(
            template.contains(&format!(
                r#"<link rel="prefetch" as="image" href="{name}">"#
            )),
            "the prefetch template must carry an inert link for {name}",
        );
    }
    // Inertness is the whole point: the links must live *inside* the
    // <template> (parsed but not loaded) and nowhere else, or a reader who
    // never opens the panel — or who has JS disabled — would fetch them for
    // nothing. So every rel=prefetch on the page must fall within the
    // template, and there must be exactly one per icon.
    assert_eq!(
        html.matches(r#"rel="prefetch""#).count(),
        template.matches(r#"rel="prefetch""#).count(),
        "prefetch links must appear only inside the inert <template>",
    );
    assert_eq!(
        template.matches(r#"rel="prefetch""#).count(),
        THEME_ICONS.len(),
    );
}

#[test]
fn theme_toggle_script_activates_the_prefetch_template_at_idle() {
    // The script reaches the inert template by the shared id and clones its
    // links into <head> to fire the prefetch. Pin the id agreement — a rename
    // on either side would silently no-op the warm-up — and that activation is
    // deferred to idle time so it never contends with first-paint work.
    assert!(
        THEME_TOGGLE_SCRIPT.contains(THEME_ICON_PREFETCH_TEMPLATE_ID),
        "theme script must look up the prefetch template by its shared id",
    );
    assert!(
        THEME_TOGGLE_SCRIPT.contains("requestIdleCallback"),
        "the prefetch activation must be scheduled off the critical path",
    );
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
            settings.contains(&format!(r#"url("{name}")"#)),
            "settings.css must reference the theme icon {name} as a mask",
        );
    }
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
            r#"<script src="{CONFIG_TOGGLE_SCRIPT_FILENAME}"></script>"#
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
