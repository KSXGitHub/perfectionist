//! The contracts the browser scripts keep: how the page loads them, the
//! shape each file holds to, and the division of labour between them —
//! markup cloned rather than built, wording left in the template, one
//! matcher for all of them deciding what a query reaches, and the two
//! libraries kept clear of the DOM so they can be run without one.
//!
//! These read the scripts as source text. A script that crossed any of
//! these lines would still drive a working page, so running it proves
//! nothing and only reading it will do.

use super::{fake_context, fake_rule};
use crate::render::{
    FILTER_BOXES_SCRIPT, FILTER_BOXES_SCRIPT_FILENAME, HIGHLIGHT_SCRIPT, HIGHLIGHT_SCRIPT_FILENAME,
    MATCH_SCRIPT, MATCH_SCRIPT_FILENAME, PAGE_SCRIPTS, RANK_SCRIPT, RANK_SCRIPT_FILENAME,
    SEARCH_OVERLAY_SCRIPT, SEARCH_OVERLAY_SCRIPT_FILENAME, render_page,
};

/// Every file the search is built from: the name it ships under, its
/// source, and the one global it declares, where it declares one. The ones
/// that declare a global are the libraries; the ones that don't are the
/// controls, and each of those reads at least one library.
const SEARCH_SCRIPTS: [(&str, &str, Option<&str>); 5] = [
    (
        MATCH_SCRIPT_FILENAME,
        MATCH_SCRIPT,
        Some("perfectionistMatch"),
    ),
    (RANK_SCRIPT_FILENAME, RANK_SCRIPT, Some("perfectionistRank")),
    (
        HIGHLIGHT_SCRIPT_FILENAME,
        HIGHLIGHT_SCRIPT,
        Some("perfectionistHighlight"),
    ),
    (FILTER_BOXES_SCRIPT_FILENAME, FILTER_BOXES_SCRIPT, None),
    (SEARCH_OVERLAY_SCRIPT_FILENAME, SEARCH_OVERLAY_SCRIPT, None),
];

#[test]
fn page_links_the_search_scripts_externally() {
    let html = render_page(&[fake_rule("only")], &fake_context());
    // Each ships as a sibling file loaded via `<script src>`, not inlined
    // — the same contract as the nav, theme and config scripts.
    for (name, _, _) in SEARCH_SCRIPTS {
        assert!(
            html.contains(&format!(r#"<script src="{name}"></script>"#)),
            "expected {name} to be referenced via <script src>",
        );
    }
}

#[test]
fn every_library_loads_before_the_scripts_that_read_it() {
    // The page loads classic scripts in order, so a file that reads
    // another's global needs that other to have run already. Every such
    // read happens during the reader's own setup — which is what makes a
    // missing library leave a control hidden rather than dead — so the
    // wrong order breaks the page outright instead of degrading.
    //
    // Which file reads which is scanned out of the sources rather than
    // listed here, so a dependency added between any two of them is
    // covered the moment it is written.
    let position = |name: &str| {
        PAGE_SCRIPTS
            .iter()
            .position(|&script| script == name)
            .unwrap_or_else(|| panic!("{name} is not in PAGE_SCRIPTS"))
    };
    for (reader, source, declares) in SEARCH_SCRIPTS {
        let code = strip_js_comments(source);
        let mut reads = 0;
        for (library, _, global) in SEARCH_SCRIPTS {
            let Some(global) = global else { continue };
            if library == reader || !code.contains(&format!("{global}.")) {
                continue;
            }
            assert!(
                position(library) < position(reader),
                "{reader} reads {global}, so {library} has to load first",
            );
            reads += 1;
        }
        // A file declaring no global of its own is one of the controls,
        // and neither can filter or search without a library. So a zero
        // here means the scan above matched nothing and left the ordering
        // unchecked, not that the control stands alone.
        if declares.is_none() {
            assert!(reads > 0, "{reader} drives a control but reads no library");
        }
    }
}

#[test]
fn search_scripts_are_each_a_single_iife() {
    // Same structural sanity check as `nav_toggle_script_is_a_single_iife`
    // — see that test for the bug it guards against. A library wraps its
    // IIFE in an assignment, so its opener reads differently.
    for (name, script, global) in SEARCH_SCRIPTS {
        let opener = match global {
            Some(global) => format!("var {global} = (function () {{"),
            None => "(function () {".to_owned(),
        };
        assert_eq!(
            script.matches(opener.as_str()).count(),
            1,
            "{name} should open exactly one IIFE",
        );
        assert_eq!(
            script.matches("})();").count(),
            1,
            "{name} should close exactly one IIFE",
        );
    }
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
    assert!(SEARCH_OVERLAY_SCRIPT.contains("resultTemplate.content.cloneNode(true)"));
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
fn every_way_out_of_a_filter_box_clears_it() {
    // Escape, the funnel and following one of the entries all dismiss a
    // box, and all three have to clear it: a box that hid while its query
    // still narrowed the list would leave the reader with entries missing
    // and nothing on screen to say why. One `closeBox` does that for all
    // three — clearing by hand on a second path is how they would drift —
    // so the count below is its definition plus the two paths that reach
    // it, `dismiss` and the entry-click handler.
    assert_eq!(
        FILTER_BOXES_SCRIPT.matches("closeBox(").count(),
        3,
        "expected one `closeBox` definition and exactly two calls",
    );
    assert_eq!(
        FILTER_BOXES_SCRIPT.matches(r#"input.value = """#).count(),
        1,
        "only `closeBox` should clear the query",
    );
}

#[test]
fn what_a_query_reaches_is_decided_only_in_the_match_library() {
    // match.js answers both questions a search asks — which targets a
    // query reaches, and how well it reaches them — and only the second is
    // a number. The first is `admits`, which reads no score. A bar on the
    // number that orders results would tie what a reader can find to how
    // the weights happen to be tuned, and would let one more character
    // push a result back over a bar it had fallen under, so a rule would
    // leave the list and come back.
    let code = strip_js_comments(MATCH_SCRIPT);
    assert!(code.contains("function admits("));
    assert!(
        code.matches("admits(").count() > 1,
        "`admits` has to be consulted, not merely defined",
    );
    // The bounds that used to answer it are gone from every script, not
    // just unread in one of them — a consumer holding one could only put
    // the old answer back in one list and not the other.
    for (name, script, _) in SEARCH_SCRIPTS {
        for bound in ["FILTER_MIN_SCORE", "SEARCH_MIN_SCORE"] {
            assert!(
                !script.contains(bound),
                "{name} still carries {bound}; what a query reaches is structural now",
            );
        }
    }
}

#[test]
fn the_search_scatters_a_name_but_keeps_a_word_in_prose() {
    // A lint name is typed from memory, so its characters may be
    // scattered; prose is typed as words, and a long enough paragraph
    // contains almost any scattered sequence. match.js offers both, and
    // rank.js has to pick the right one per field or every rule on the
    // page matches every query.
    assert!(MATCH_SCRIPT.contains("function matchFuzzy("));
    assert!(MATCH_SCRIPT.contains("function matchPhrase("));
    // A separator spelled differently and a word ending differently are
    // met by both, so a near miss reaches a paragraph as readily as a
    // name. The two that let the query come apart — characters dropped
    // out of a word, words arriving out of order — are the fuzzy one's
    // alone, and a paragraph long enough carries either by accident.
    assert!(MATCH_SCRIPT.contains("function matchRespaced("));
    assert!(MATCH_SCRIPT.contains("function matchVariants("));
    for scan in ["matchScattered(", "matchReordered("] {
        assert_eq!(
            MATCH_SCRIPT.matches(scan).count(),
            2,
            "{scan} should have one definition and one caller, in matchFuzzy",
        );
    }
    // Both consumers reach the matchers through the one global, whether
    // they call them where they stand or bind them to a local first.
    assert!(RANK_SCRIPT.contains("perfectionistMatch.matchFuzzy"));
    assert!(RANK_SCRIPT.contains("perfectionistMatch.matchPhrase"));
    // The filter boxes match names only, so they never want the prose
    // variant.
    assert!(FILTER_BOXES_SCRIPT.contains("perfectionistMatch.matchFuzzy"));
    assert!(!FILTER_BOXES_SCRIPT.contains("matchPhrase"));
}

#[test]
fn the_libraries_touch_no_dom() {
    // match.js and rank.js are the page's two pure libraries, and that is
    // not an accident of how they happen to be written: it is what lets
    // tools/gen-docs/tests/ load them at all, which is the only way the
    // scoring and the weights get checked. A DOM reference in either would
    // end that silently — the page would go on working, and the unit tests
    // would start wanting a fake document.
    //
    // Comments are stripped first: both files discuss the DOM at length
    // while touching none of it.
    for (name, script) in [("match.js", MATCH_SCRIPT), ("rank.js", RANK_SCRIPT)] {
        let code = strip_js_comments(script);
        for api in [
            "document",
            "window",
            "HTMLElement",
            "appendChild",
            "querySelector",
            "addEventListener",
            "localStorage",
        ] {
            assert!(
                !code.contains(api),
                "{name} must stay loadable without a browser, but reaches for `{api}`",
            );
        }
    }
    // The DOM work they were split from still exists, in the one file that
    // owns it.
    assert!(HIGHLIGHT_SCRIPT.contains("document.createElement"));
}

/// A copy of `js` with `//` line comments and `/* */` block comments
/// removed, so a scan for an API name can't be fooled by prose about it.
/// Crude on purpose: it knows nothing of string or regex literals, which
/// is adequate for scanning our own source and wrong for anything else.
fn strip_js_comments(js: &str) -> String {
    let mut out = String::new();
    let mut rest = js;
    loop {
        let line = rest.find("//");
        let block = rest.find("/*");
        let (start, end_of) = match (line, block) {
            (Some(l), Some(b)) if l < b => (l, rest[l..].find('\n').map(|i| l + i)),
            (Some(_), Some(b)) => (b, rest[b..].find("*/").map(|i| b + i + "*/".len())),
            (Some(l), None) => (l, rest[l..].find('\n').map(|i| l + i)),
            (None, Some(b)) => (b, rest[b..].find("*/").map(|i| b + i + "*/".len())),
            (None, None) => break,
        };
        out.push_str(&rest[..start]);
        match end_of {
            Some(end) => rest = &rest[end..],
            None => return out, // unterminated; the tail is all comment
        }
    }
    out.push_str(rest);
    out
}
