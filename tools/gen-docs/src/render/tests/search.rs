//! The page's search and filter affordances: the magnifier and the two
//! funnels, the inert `<template>`s their markup lives in, the scripts
//! that clone and drive them, and the stylesheet rules that paint them.
//!
//! The fixtures and the stylesheet lookup these share with the rest of
//! the page's tests stay in the parent module.

use super::{fake_context, fake_rule, stylesheet};
use crate::render::{
    FILTER_BOXES_SCRIPT, FILTER_BOXES_SCRIPT_FILENAME, MATCH_SCRIPT, MATCH_SCRIPT_FILENAME,
    PAGE_SCRIPTS, SEARCH_ICONS, SEARCH_OVERLAY_ID, SEARCH_OVERLAY_SCRIPT,
    SEARCH_OVERLAY_SCRIPT_FILENAME, SEARCH_OVERLAY_TEMPLATE_ID, SEARCH_RESULT_TEMPLATE_ID,
    render_page,
};

/// The slice of `html` spanning the `<template id="...">` with this id,
/// closing tag included. The catalogue's JS-driven UI lives inside these,
/// so a test that wants to pin that markup — or to prove it appears
/// nowhere else — needs the boundaries.
fn template_with_id<'a>(html: &'a str, id: &str) -> &'a str {
    let open = format!(r#"<template id="{id}">"#);
    let start = html
        .find(&open)
        .unwrap_or_else(|| panic!("no <template id={id}> in the page"));
    let end = start
        + html[start..]
            .find("</template>")
            .unwrap_or_else(|| panic!("<template id={id}> is unterminated"))
        + "</template>".len();
    &html[start..end]
}

#[test]
fn page_emits_search_toggle_hidden_and_pointing_at_the_overlay() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // The magnifier mirrors the gear: a plain <button> driven by
    // `aria-expanded` and emitted with the HTML `hidden` attribute, so it
    // appears only once search_overlay.js has cloned the overlay in and
    // wired up its handlers. Anchor to the opening tag so a refactor that
    // drops `hidden` is caught.
    assert!(html.contains(&format!(
        r#"<button class="search-toggle" type="button" hidden aria-controls="{SEARCH_OVERLAY_ID}" aria-expanded="false""#
    )));
    assert!(html.contains(r#"aria-label="Search lints""#));
    // The id the button names has to be the one the overlay markup
    // carries, or the button points at nothing once the overlay is in the
    // page. Both read it from the same constant, so pin that they still
    // agree in the rendered output.
    let template = template_with_id(&html, SEARCH_OVERLAY_TEMPLATE_ID);
    assert!(
        template.contains(&format!(
            r#"<div class="search-overlay" id="{SEARCH_OVERLAY_ID}" hidden>"#
        )),
        "the overlay must carry the id its button names, and start hidden",
    );
}

#[test]
fn search_overlay_markup_lives_only_inside_its_template() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // A <template>'s contents are parsed but kept out of the document:
    // nothing renders, nothing is focusable, assistive tech never reaches
    // them, and `querySelector` does not descend into them. That is what
    // keeps "no overlay without JavaScript" true while the markup itself
    // stays here. It only holds while the overlay markup appears nowhere
    // else on the page, so count rather than merely find.
    let template = template_with_id(&html, SEARCH_OVERLAY_TEMPLATE_ID);
    for needle in [
        r#"class="search-overlay""#,
        r#"class="search-dialog""#,
        r#"class="search-box""#,
        r#"class="search-input""#,
        r#"class="search-close""#,
        r#"class="search-results""#,
    ] {
        assert_eq!(template.matches(needle).count(), 1, "{needle} is missing");
        assert_eq!(
            html.matches(needle).count(),
            1,
            "{needle} must appear only inside the overlay template",
        );
    }
    // The dialog's modal semantics, and the close button's accessible name
    // ("Close" alone — the ✕ beside it is decoration), are markup, so they
    // are pinned here rather than left to a browser check.
    assert!(template.contains(r#"role="dialog" aria-modal="true" aria-label="Search lints""#));
    assert!(
        template.contains(
            r#"<span class="search-close-glyph" aria-hidden="true">✕</span>Close</button>"#
        ),
    );
}

#[test]
fn an_empty_results_list_says_why_it_is_empty() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    let template = template_with_id(&html, SEARCH_OVERLAY_TEMPLATE_ID);
    // A blank panel reads the same whether nothing has been typed or
    // nothing matched, so each case has its own wording — written here
    // rather than in the script, which owns only which one shows.
    assert!(
        template.contains(
            r#"<p class="search-empty search-empty-prompt" role="status">Type to search lint names and documentation.</p>"#
        ),
        "the overlay must carry the prompt shown before anything is typed, got: {template}",
    );
    assert!(
        template.contains(
            r#"<p class="search-empty search-empty-no-match" role="status" hidden>No lint matches that search.</p>"#
        ),
        "the overlay must carry the wording for a query that matches nothing, got: {template}",
    );
    // The overlay opens on the prompt: nothing has been typed, so the
    // list and the no-match wording are the two that start hidden. An
    // empty `<ul>` left showing would claim the dialog's whole remaining
    // height and push the prompt below it.
    assert!(
        template.contains(r#"<ul class="search-results" aria-label="Search results" hidden>"#),
        "the results list must start hidden, got: {template}",
    );
    assert_eq!(
        template.matches(r#"class="search-empty "#).count(),
        2,
        "exactly two empty-state messages, got: {template}",
    );
    assert_eq!(
        template.matches(r#"role="status" hidden>"#).count(),
        1,
        "exactly one of the two must start hidden, got: {template}",
    );
}

#[test]
fn the_script_chooses_the_empty_state_but_does_not_word_it() {
    // The wording is markup; the script owns only which of the three
    // panels shows. A string of prose here would be text that escaped the
    // Rust template, where the rest of the page's text lives.
    assert!(SEARCH_OVERLAY_SCRIPT.contains("function showOnly("));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("showOnly(emptyPrompt)"));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("showOnly(emptyNoMatch)"));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("showOnly(results)"));
    for wording in ["Type to search", "No lint matches"] {
        assert!(
            !SEARCH_OVERLAY_SCRIPT.contains(wording),
            "the script must not carry the empty-state wording ({wording})",
        );
    }
}

#[test]
fn the_close_button_sits_in_the_results_corner_not_the_search_box() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    let template = template_with_id(&html, SEARCH_OVERLAY_TEMPLATE_ID);
    // The search box holds the input and nothing else, so it and the
    // results list still account for the whole of the dialog's height
    // between them.
    assert!(
        template.contains(r#"<div class="search-box"><input class="search-input""#),
        "the search box must hold the input alone",
    );
    // The button comes after the results list and the two messages that
    // stand in for it, as the dialog's last child, which is what lets
    // search.css park it in the results' bottom corner and what puts it
    // after the results in the tab order. Pinned as an ordering rather
    // than an adjacency so a further panel between them doesn't break it.
    let results = template
        .find(r#"<ul class="search-results""#)
        .expect("results list missing");
    let button = template
        .find(r#"<button class="search-close""#)
        .expect("close button missing");
    assert!(
        results < button,
        "the close button must follow the results list, got: {template}",
    );
    assert!(
        template.contains(r#"</button></div></div></template>"#),
        "the close button must be the dialog's last child, got: {template}",
    );
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
fn search_result_template_carries_the_shape_the_script_fills_in() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // One result's markup, cloned per result. It is a template of its own
    // rather than one nested in the results list because that list is
    // emptied on every keystroke and would take its own blueprint with it.
    let template = template_with_id(&html, SEARCH_RESULT_TEMPLATE_ID);
    assert!(
        template.contains(
            r#"<li><a class="search-result"><code class="search-result-name"></code><span class="search-result-text"></span></a></li>"#
        ),
        "the result blueprint must carry the name and text slots, got: {template}",
    );
    // No `href`: the blueprint points nowhere until a result fills it in.
    assert!(
        !template.contains("href="),
        "the result blueprint must not carry an href",
    );
}

#[test]
fn page_emits_a_hidden_filter_toggle_and_a_templated_container_per_list() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // One pair per filtered list. The toggle carries the `<kind>-filter-
    // toggle` class filter_boxes.js selects on and points at the
    // container's id; the container carries the matching
    // `<kind>-filter-container` class and holds nothing but the inert
    // `<template>` that is the box's blueprint, so a page whose script
    // never runs lays out as though the feature did not exist.
    for (kind, label) in [
        ("index", "Filter the index by lint name"),
        ("nav", "Filter the navigation by lint name"),
    ] {
        assert!(
            html.contains(&format!(
                r#"<button class="filter-toggle {kind}-filter-toggle" type="button" hidden aria-controls="{kind}-filter" aria-expanded="false" aria-label="{label}""#
            )),
            "expected a hidden {kind} filter toggle pointing at its container",
        );
        assert!(
            html.contains(&format!(
                r#"<div class="filter-container {kind}-filter-container" id="{kind}-filter"><template><div class="filter-box" hidden>"#
            )),
            "expected the {kind} filter container to hold a template whose box starts hidden",
        );
        // The funnel and the input it opens announce as one thing, so the
        // accessible name has to be the same on both.
        assert!(
            html.contains(&format!(
                r#"<input class="filter-input" type="search" placeholder="Filter by name…" aria-label="{label}""#
            )),
            "the {kind} filter input must share its funnel's accessible name",
        );
    }
}

#[test]
fn query_inputs_carry_the_attributes_touch_keyboards_need() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // A phone or tablet keyboard otherwise capitalises the first letter
    // and autocorrects a half-typed lint name into a dictionary word,
    // neither of which can match a snake_case identifier. `type="search"`
    // also earns the platform's search affordances — a clear button on
    // desktop, a dedicated key on a touch keyboard. Every query input on
    // the page wants all of it.
    let keyboard = r#"autocapitalize="none" autocorrect="off" autocomplete="off" enterkeyhint="search" spellcheck="false""#;
    let inputs = html.matches(r#"<input class="search-input""#).count()
        + html.matches(r#"<input class="filter-input""#).count();
    assert_eq!(inputs, 3, "expected the search input and two filter inputs");
    assert_eq!(html.matches(keyboard).count(), inputs);
    assert_eq!(
        html.matches(r#"<input class="search-input" type="search""#)
            .count()
            + html
                .matches(r#"<input class="filter-input" type="search""#)
                .count(),
        inputs,
        "every query input must be a search input",
    );
}

#[test]
fn index_filter_container_sits_between_the_heading_and_the_table() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // The heading itself is the flex row that right-aligns the funnel
    // against the word "Index" (search.css keys off `.index-heading`), and
    // the box the funnel opens belongs between that heading and the table
    // it filters. Pin the order so a refactor can't float the input
    // somewhere else on the page.
    let heading = html
        .find(r#"<h2 class="index-heading">Index<button class="filter-toggle index-filter-toggle""#)
        .expect("the Index heading must carry its filter toggle on the same line");
    let container = html
        .find(r#"<div class="filter-container index-filter-container""#)
        .expect("index filter container missing");
    let table = html
        .find(r#"<table class="index">"#)
        .expect("index table missing");
    assert!(
        heading < container && container < table,
        "the index filter container must sit between the Index heading and the table",
    );
}

#[test]
fn nav_filter_container_sits_under_the_sidebar_header() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // The funnel is the header's last item, right-aligned against the
    // drawer's title, and the box it opens sits directly below the header
    // — before the list it filters.
    assert!(
        html.contains(r#"perfectionist lints</a><button class="filter-toggle nav-filter-toggle""#),
        "the sidebar header's filter toggle must follow the title, right-aligned against it",
    );
    assert!(
        html.contains(
            r#"</div><div class="filter-container nav-filter-container" id="nav-filter"><template>"#
        ),
        "the nav filter container must follow the sidebar header and hold its blueprint",
    );
    let container = html
        .find(r#"<div class="filter-container nav-filter-container""#)
        .expect("nav filter container missing");
    let list = html
        .find(r#"<ul class="nav-sidebar-list">"#)
        .expect("nav sidebar list missing");
    assert!(
        container < list,
        "the nav filter container must sit before the list it filters",
    );
}

#[test]
fn the_search_scripts_clone_markup_rather_than_build_it() {
    // The page's markup belongs in the Rust template, where it is reviewed
    // alongside the rest of the page and tested without a browser. These
    // two scripts therefore own behaviour only: every element they put in
    // the page comes from cloning a `<template>`, so neither has any
    // business calling `createElement`.
    for (name, script) in [
        ("filter_boxes.js", FILTER_BOXES_SCRIPT),
        ("search_overlay.js", SEARCH_OVERLAY_SCRIPT),
    ] {
        assert!(
            script.contains("content.cloneNode(true)"),
            "{name} must clone its markup from a <template>",
        );
        assert!(
            !script.contains("createElement"),
            "{name} must not build markup; clone the <template> instead",
        );
    }
    // match.js is the exception, and deliberately: it wraps matched
    // substrings in `<mark>` and re-inserts the `<wbr>` break
    // opportunities a lint name needs, which is a run of elements whose
    // number and placement are decided per query. That is text rendering,
    // not page structure, and no fixed blueprint can express it.
    assert!(MATCH_SCRIPT.contains("createElement"));
}

#[test]
fn the_search_scripts_clone_only_where_repetition_is_the_point() {
    // Cloning the overlay and the filter boxes is setup, not a per-open
    // step: each enters the page once, and showing or hiding it afterwards
    // is nothing but the `hidden` attribute. A script that re-cloned on
    // every open would leak a copy per click and lose whatever state the
    // last one held, so pin how many clones each file performs.
    assert_eq!(
        FILTER_BOXES_SCRIPT.matches("cloneNode").count(),
        1,
        "filter_boxes.js must clone one box per container and no more",
    );
    assert_eq!(
        SEARCH_OVERLAY_SCRIPT.matches("cloneNode").count(),
        2,
        "search_overlay.js must clone the overlay and the result blueprint, and nothing else",
    );
    // Which of the two is which: the overlay once at setup, the result
    // blueprint once per result — the one place repetition is the point,
    // since the list is rebuilt on every keystroke.
    assert!(SEARCH_OVERLAY_SCRIPT.contains("overlayBlueprint.content.cloneNode(true)"));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("resultTemplate.content.cloneNode(true)"));
}

#[test]
fn page_links_the_search_scripts_externally() {
    let html = render_page(&[fake_rule("only")], &fake_context());
    // All three ship as sibling files loaded via `<script src>`, not
    // inlined — the same contract as the nav, theme and config scripts.
    for name in [
        MATCH_SCRIPT_FILENAME,
        FILTER_BOXES_SCRIPT_FILENAME,
        SEARCH_OVERLAY_SCRIPT_FILENAME,
    ] {
        assert!(
            html.contains(&format!(r#"<script src="{name}"></script>"#)),
            "expected {name} to be referenced via <script src>",
        );
    }
}

#[test]
fn match_library_loads_before_the_scripts_that_read_it() {
    // The page loads classic scripts in order, and both consumers read
    // `perfectionistMatch` during their own setup (that read is what makes
    // a missing library leave their controls hidden rather than dead), so
    // the library has to have run first.
    let position = |name: &str| {
        PAGE_SCRIPTS
            .iter()
            .position(|&script| script == name)
            .unwrap_or_else(|| panic!("{name} is not in PAGE_SCRIPTS"))
    };
    let library = position(MATCH_SCRIPT_FILENAME);
    assert!(library < position(FILTER_BOXES_SCRIPT_FILENAME));
    assert!(library < position(SEARCH_OVERLAY_SCRIPT_FILENAME));
}

#[test]
fn the_two_filter_boxes_share_one_implementation() {
    // "The two filterings use completely identical logic": one
    // `installFilter`, defined once and called once per list. A second
    // definition — the obvious way to let the two drift — would push the
    // count past three.
    assert_eq!(
        FILTER_BOXES_SCRIPT.matches("installFilter(").count(),
        3,
        "expected one `installFilter` definition and exactly two calls",
    );
}

#[test]
fn the_score_bounds_live_only_in_the_match_library() {
    // One bound per kind of matching, held in match.js, read by its
    // consumers. A consumer that hard-coded a number instead could drift
    // from the other filter box, which the bound exists to prevent.
    assert!(MATCH_SCRIPT.contains("FILTER_MIN_SCORE:"));
    assert!(MATCH_SCRIPT.contains("SEARCH_MIN_SCORE:"));
    assert!(FILTER_BOXES_SCRIPT.contains("perfectionistMatch.FILTER_MIN_SCORE"));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("perfectionistMatch.SEARCH_MIN_SCORE"));
    assert!(
        !FILTER_BOXES_SCRIPT.contains("SEARCH_MIN_SCORE"),
        "the filter boxes must use the filter bound, not the search one",
    );
    assert!(
        !SEARCH_OVERLAY_SCRIPT.contains("FILTER_MIN_SCORE"),
        "the search must use the search bound, not the filter one",
    );
}

#[test]
fn the_search_matches_names_loosely_and_prose_verbatim() {
    // A lint name is typed from memory, so its characters may be
    // scattered; prose is typed as words, and a long enough paragraph
    // contains almost any scattered sequence. The library offers both, and
    // the overlay has to pick the right one per field or every rule on the
    // page matches every query.
    assert!(MATCH_SCRIPT.contains("function matchFuzzy("));
    assert!(MATCH_SCRIPT.contains("function matchPhrase("));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("perfectionistMatch.matchFuzzy("));
    assert!(SEARCH_OVERLAY_SCRIPT.contains("perfectionistMatch.matchPhrase("));
    // The filter boxes match names only, so they never want the prose
    // variant.
    assert!(FILTER_BOXES_SCRIPT.contains("perfectionistMatch.matchFuzzy("));
    assert!(!FILTER_BOXES_SCRIPT.contains("matchPhrase"));
}

#[test]
fn search_scripts_are_each_a_single_iife() {
    // Same structural sanity check as `nav_toggle_script_is_a_single_iife`
    // — see that test for the bug it guards against. match.js wraps its
    // IIFE in an assignment, so its opener reads differently.
    assert_eq!(
        MATCH_SCRIPT
            .matches("var perfectionistMatch = (function () {")
            .count(),
        1,
    );
    assert_eq!(MATCH_SCRIPT.matches("})();").count(), 1);
    assert_eq!(FILTER_BOXES_SCRIPT.matches("(function () {").count(), 1);
    assert_eq!(FILTER_BOXES_SCRIPT.matches("})();").count(), 1);
    assert_eq!(SEARCH_OVERLAY_SCRIPT.matches("(function () {").count(), 1);
    assert_eq!(SEARCH_OVERLAY_SCRIPT.matches("})();").count(), 1);
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
