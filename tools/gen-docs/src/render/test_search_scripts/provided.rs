//! What the scripts take rather than make. The markup is cloned from a
//! `<template>` the page renders, the wording of each empty state stays
//! in that template, and moving between results is the browser's own
//! focus rather than a synthesised keystroke. A script that built any of
//! it instead would work, which is why only reading it will do.

use crate::render::{FILTER_BOXES_SCRIPT, HIGHLIGHT_SCRIPT, SEARCH_OVERLAY_SCRIPT};

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
    // highlight.js is the exception, and deliberately: it wraps matched
    // substrings in `<mark>` and re-inserts the `<wbr>` break
    // opportunities a lint name needs, which is a run of elements whose
    // number and placement are decided per query. That is text rendering,
    // not page structure, and no fixed blueprint can express it.
    assert!(HIGHLIGHT_SCRIPT.contains("createElement"));
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
    assert!(SEARCH_OVERLAY_SCRIPT.contains("resultBlueprint.content.cloneNode(true)"));
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
fn the_overlay_moves_focus_rather_than_faking_a_keystroke() {
    // Up and Down walk the overlay's focusable elements by focusing one.
    // The obvious alternative — building a Tab `KeyboardEvent` and
    // dispatching it — does nothing whatever: an event constructed in
    // script is untrusted, and an untrusted event performs no default
    // action, so focus stays where it was. It fails silently, which is
    // why it is worth pinning that it is not what is here.
    assert!(
        SEARCH_OVERLAY_SCRIPT.contains(".focus()"),
        "the overlay should move focus by focusing an element",
    );
    assert!(
        !SEARCH_OVERLAY_SCRIPT.contains("new KeyboardEvent"),
        "a dispatched Tab moves nothing; focus the element instead",
    );
}
