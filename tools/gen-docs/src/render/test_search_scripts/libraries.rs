//! The division of labour between the libraries: which of them decides
//! what a query reaches, which matcher each consumer asks for, and the
//! line that keeps every one of them loadable without a browser.

use super::{SEARCH_SCRIPTS, strip_js_comments};
use crate::render::{
    FILTER_BOXES_SCRIPT, HIGHLIGHT_SCRIPT, MATCH_ADMIT_SCRIPT, MATCH_ADMIT_SCRIPT_FILENAME,
    MATCH_SCORE_SCRIPT, MATCH_SCORE_SCRIPT_FILENAME, MATCH_SCRIPT, MATCH_SCRIPT_FILENAME,
    MATCH_TEXT_SCRIPT, MATCH_TEXT_SCRIPT_FILENAME, MATCH_TIERS_SCRIPT, MATCH_TIERS_SCRIPT_FILENAME,
    RANK_SCRIPT, RANK_SCRIPT_FILENAME,
};

#[test]
fn what_a_query_reaches_is_decided_only_in_the_match_library() {
    // match.js answers both questions a search asks — which targets a
    // query reaches, and how well it reaches them — and only the second is
    // a number. The first is `admits`, which reads no score. A bar on the
    // number that orders results would tie what a reader can find to how
    // the weights happen to be tuned, and would let one more character
    // push a result back over a bar it had fallen under, so a rule would
    // leave the list and come back.
    assert!(strip_js_comments(MATCH_ADMIT_SCRIPT).contains("function admits("));
    let consulted = [MATCH_TIERS_SCRIPT, MATCH_SCRIPT]
        .map(strip_js_comments)
        .iter()
        .map(|code| code.matches("admits(").count())
        .sum::<usize>();
    assert!(
        consulted > 1,
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
    assert!(MATCH_TIERS_SCRIPT.contains("function matchRespaced("));
    assert!(MATCH_TIERS_SCRIPT.contains("function matchVariants("));
    for scan in ["matchScattered(", "matchReordered("] {
        assert_eq!(
            MATCH_TIERS_SCRIPT.matches(scan).count(),
            1,
            "{scan} should be defined once, in the tiers",
        );
        assert!(
            MATCH_SCRIPT.contains(&format!("perfectionistMatchTiers.{scan}").replace('(', "")),
            "{scan} should reach matchFuzzy through the tiers' global",
        );
        assert_eq!(
            MATCH_SCRIPT.matches(scan).count(),
            1,
            "{scan} should be called once, in matchFuzzy, and nowhere else",
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
    for (name, script) in [
        (MATCH_TEXT_SCRIPT_FILENAME, MATCH_TEXT_SCRIPT),
        (MATCH_SCORE_SCRIPT_FILENAME, MATCH_SCORE_SCRIPT),
        (MATCH_ADMIT_SCRIPT_FILENAME, MATCH_ADMIT_SCRIPT),
        (MATCH_TIERS_SCRIPT_FILENAME, MATCH_TIERS_SCRIPT),
        (MATCH_SCRIPT_FILENAME, MATCH_SCRIPT),
        (RANK_SCRIPT_FILENAME, RANK_SCRIPT),
    ] {
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
