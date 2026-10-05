//! What the stylesheets have to say about the search and filter
//! affordances: the icons they mask, the corner the close button is
//! folded into, the backdrop's transparency, and that every surface is
//! coloured in both theme layers.
//!
//! No rendered string carries any of this, so without these tests
//! `just all` would never look at it at all.

use super::stylesheet;
use crate::render::SEARCH_ICONS;

/// The declarations of the rule `css` opens with `selector`, which must
/// be written with its brace (`.foo {`) so a longer selector sharing the
/// prefix can't match.
fn declaration_block<'a>(css: &'a str, selector: &str) -> &'a str {
    let start = css
        .find(selector)
        .unwrap_or_else(|| panic!("no `{selector}` rule"));
    let end = start
        + css[start..]
            .find('}')
            .unwrap_or_else(|| panic!("`{selector}` is unterminated"));
    &css[start..end]
}

#[test]
fn search_css_masks_the_bundled_control_icons() {
    // The CSS `url(...)` and the written filenames must agree, or the
    // icons 404 and both buttons render blank.
    let search = stylesheet("search.css");
    for (name, _) in SEARCH_ICONS {
        assert!(
            search.contains(&format!(r#"url("{name}")"#)),
            "search.css must reference the control icon {name} as a mask",
        );
    }
    for (name, content) in SEARCH_ICONS {
        assert!(
            content.contains("Octicons") && content.contains("MIT"),
            "the bundled {name} must retain its Octicons MIT attribution",
        );
    }
}

#[test]
fn the_close_button_is_fused_into_the_dialog_corner() {
    // The part of the design the markup cannot carry: parked against the
    // dialog's bottom-right — the dialog's, not the list's, so it never
    // scrolls out of reach — bordered and rounded only on the two edges
    // facing the dialog's interior, so it reads as the corner folding up
    // rather than a button dropped on one, and half-transparent until
    // hovered, which is what licenses it to cover the last row of
    // results.
    let search = stylesheet("search.css");
    let rule = declaration_block(search, ".search-close {");
    for declaration in [
        "position: absolute",
        "right: 0",
        "bottom: 0",
        "border: 0",
        "border-top: 1px solid",
        "border-left: 1px solid",
        "border-radius: 6px 0 0 0",
        "opacity: 0.55",
    ] {
        assert!(
            rule.contains(declaration),
            "search.css's .search-close must set `{declaration}`, got: {rule}",
        );
    }
    assert!(
        search.contains(".search-close:hover,\n.search-close:focus-visible {\n  opacity: 1;\n}"),
        "hover and keyboard focus must bring the button to full opacity",
    );
    // Without this the corner offsets resolve against the viewport.
    assert!(
        declaration_block(search, ".search-dialog {").contains("position: relative"),
        "the dialog must be the close button's positioning context",
    );
}

#[test]
fn the_search_overlay_backdrop_is_partly_transparent() {
    // The overlay dims the page rather than replacing it, so the reader
    // keeps their place while they search. An opaque backdrop would read
    // as a different page.
    for name in ["light.css", "dark.css"] {
        let sheet = stylesheet(name);
        let start = sheet
            .find(".search-overlay {")
            .unwrap_or_else(|| panic!("{name} must colour .search-overlay"));
        let rule = &sheet[start..start + sheet[start..].find('}').expect("unterminated rule")];
        assert!(
            rule.contains("rgba("),
            "{name}'s .search-overlay background must carry an alpha channel",
        );
    }
}

#[test]
fn search_and_filter_colours_live_in_both_theme_layers() {
    // Every surface the feature adds is themed in the colour layer, not
    // the structural sheet. Both layers must carry each selector, or the
    // surface silently loses its colours in one theme — the dark one being
    // the easy half to forget, since it is written twice over.
    let light = stylesheet("light.css");
    let dark = stylesheet("dark.css");
    for selector in [
        ".search-toggle",
        ".filter-toggle",
        ".filter-input",
        ".search-overlay",
        ".search-dialog",
        ".search-input",
        ".search-close",
        ".search-result",
        ".match-highlight",
    ] {
        assert!(light.contains(selector), "light.css must colour {selector}");
        // Once per dark tier: system preference, then explicit override.
        assert!(
            dark.contains(&format!(
                r#"html:not([color-scheme-override="light"]) {selector} "#
            )),
            "dark.css must colour {selector} under the system-preference tier",
        );
        assert!(
            dark.contains(&format!(
                r#"html[color-scheme-override="dark"] {selector} "#
            )),
            "dark.css must colour {selector} under the explicit-Dark tier",
        );
    }
}
