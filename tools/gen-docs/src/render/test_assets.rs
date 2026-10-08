//! The files the page links and what they say: one `<link>` per
//! stylesheet and one preload per script, the font and the icons those
//! sheets reach for, and the division between the structural sheets and
//! the two colour layers.

use super::test_fixtures::{fake_context, fake_rule, stylesheet};
use super::{
    HIGHLIGHT_CSS_DARK_FILENAME, HIGHLIGHT_CSS_LIGHT_FILENAME, PAGE_SCRIPTS, RULE_ANCHOR_ICON,
    RULE_ANCHOR_ICON_FILENAME, STYLESHEETS, render_page,
};
use crate::fonts::DOWNLOADS;

#[test]
fn page_links_each_stylesheet_individually() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // Every static sheet gets its own `<link>`, in slice order,
    // and the runtime-generated highlight sheet is linked last so
    // its cascade position matches the old single `<style>`.
    for &(name, _) in STYLESHEETS {
        assert!(
            html.contains(&format!(r#"<link rel="stylesheet" href="{name}">"#)),
            "expected a dedicated <link> for {name}",
        );
    }
    // Both runtime-generated highlight sheets are linked — the page now
    // emits two, so a regression that drops the dark one must fail here.
    assert!(
        html.contains(&format!(
            r#"<link rel="stylesheet" href="{HIGHLIGHT_CSS_LIGHT_FILENAME}">"#
        )),
        "expected a dedicated <link> for the light highlight CSS",
    );
    assert!(
        html.contains(&format!(
            r#"<link rel="stylesheet" href="{HIGHLIGHT_CSS_DARK_FILENAME}">"#
        )),
        "expected a dedicated <link> for the dark highlight CSS",
    );
    // Nothing is inlined any more: no `<style>` block survives.
    assert!(
        !html.contains("<style"),
        "stylesheets must be external; no inline <style> should remain",
    );
}

#[test]
fn page_preloads_each_script_in_head() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // Every page script gets a `<link rel="preload" as="script">` so the
    // browser fetches it in parallel with the stylesheets instead of
    // waiting until it parses to the `<script src>` tags at the foot of
    // <body>. Pin the exact tag for each script in `PAGE_SCRIPTS`.
    for &src in PAGE_SCRIPTS {
        assert!(
            html.contains(&format!(r#"<link rel="preload" as="script" href="{src}">"#)),
            "expected a <link rel=preload as=script> for {src}",
        );
    }
    // The preload hints must live in <head>, ahead of <body>, or they
    // can't front-run the foot-of-body `<script src>` discovery.
    let head_end = html.find("</head>").expect("no </head>");
    let first_preload = html
        .find(r#"<link rel="preload""#)
        .expect("no preload link emitted");
    assert!(
        first_preload < head_end,
        "preload hints must be emitted inside <head>",
    );
    // Both forms reference the same files, so the count of preload links
    // matches the count of loaded scripts — a script added to one loop
    // but not the other (were they ever to diverge) would skew this.
    assert_eq!(
        html.matches(r#"<link rel="preload" as="script""#).count(),
        PAGE_SCRIPTS.len(),
    );
    assert_eq!(html.matches("<script src=").count(), PAGE_SCRIPTS.len());
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
fn base_css_font_face_references_the_downloaded_font() {
    // The font the build downloads and links beside index.html must be
    // reachable through a `@font-face` `url(...)` whose name matches it,
    // or the font 404s and the page silently drops back to the system
    // sans. `local(...)` must come first so an installed Cantarell wins
    // over a download.
    let base = stylesheet("base.css");
    assert!(
        base.contains(r#"local("Cantarell")"#),
        "base.css must prefer a locally-installed Cantarell before downloading",
    );
    let font = DOWNLOADS
        .iter()
        .map(|&(name, _)| name)
        .find(|name| name.ends_with(".otf"))
        .expect("DOWNLOADS must include the .otf font");
    assert!(
        base.contains(&format!(r#"url("{font}")"#)),
        r#"base.css must reference the downloaded font as url("{font}")"#,
    );
    assert!(
        base.contains(r#"format("opentype")"#),
        "base.css must declare the OpenType format for the @font-face src",
    );
    assert!(
        base.contains(r#"font-family: "Cantarell""#),
        "base.css `body` must set Cantarell as the first font family",
    );
}

#[test]
fn pseudo_icons_opt_out_of_the_body_font() {
    // The gear, hamburger, and close ✕ must not inherit the body's
    // Cantarell. They keep the system font stack the page used before
    // Cantarell was introduced, so they render exactly as they always
    // did rather than in (or falling back from) Cantarell.
    let icon_stack = "font-family: -apple-system, BlinkMacSystemFont, system-ui, sans-serif;";
    assert!(
        stylesheet("nav.css").matches(icon_stack).count() >= 2,
        "nav.css must keep the hamburger and close ✕ on the system font stack, off Cantarell",
    );
    assert!(
        stylesheet("settings.css").contains(icon_stack),
        "settings.css must keep the gear on the system font stack, off Cantarell",
    );
    assert!(
        stylesheet("search.css").contains(icon_stack),
        "search.css must keep the search overlay's close ✕ on the system font stack, \
         off Cantarell",
    );
}

#[test]
fn style_references_rule_anchor_icon() {
    // The CSS `url(...)` and the written filename must agree, or the
    // icon 404s.
    let expected = format!(r#"url("{RULE_ANCHOR_ICON_FILENAME}")"#);
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
fn page_does_not_inline_the_anchor_icon_svg() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    assert!(
        !html.contains("<svg"),
        "the heading-anchor icon must stay an external resource, not inlined SVG",
    );
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

#[test]
fn structural_sheets_carry_no_literal_colours() {
    // The colours were extracted out of the structural sheets; they
    // should hold layout/sizing only, so a colour change is a one-file
    // edit in the colour layer. `currentColor` (a structural reference,
    // not a theme value) and `#id` selectors like `#catalogue` are
    // fine — only literal `#rrggbb` hexes and `rgb(`/`rgba(` are colours.
    for name in [
        "base.css",
        "nav.css",
        "rules.css",
        "search.css",
        "settings.css",
    ] {
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
                .is_some_and(|run| run.iter().all(u8::is_ascii_hexdigit));
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
