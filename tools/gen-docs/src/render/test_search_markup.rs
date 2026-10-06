//! What the Rust template renders for the search overlay and the two
//! filter boxes: the buttons, the inert `<template>`s their markup
//! lives in, and where each sits among the page's other elements.
//!
//! All of it is checked against the rendered string, so a change to the
//! markup shows up without a browser.

use super::test_fixtures::{fake_context, fake_rule};
use super::{
    SEARCH_OVERLAY_ID, SEARCH_OVERLAY_TEMPLATE_ID, SEARCH_RESULT_TEMPLATE_ID, render_page,
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
