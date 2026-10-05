//! The contracts the three browser scripts keep: how the page loads
//! them, the shape each file holds to, and the division of labour
//! between them — markup cloned rather than built, wording left in the
//! template, one matcher and one pair of score bounds for all of them.
//!
//! These read the scripts as source text. A script that crossed any of
//! these lines would still drive a working page, so running it proves
//! nothing and only reading it will do.

use crate::render::tests::{fake_context, fake_rule};
use crate::render::{
    FILTER_BOXES_SCRIPT, FILTER_BOXES_SCRIPT_FILENAME, MATCH_SCRIPT, MATCH_SCRIPT_FILENAME,
    PAGE_SCRIPTS, SEARCH_OVERLAY_SCRIPT, SEARCH_OVERLAY_SCRIPT_FILENAME, render_page,
};

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
