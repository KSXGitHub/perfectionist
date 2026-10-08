//! One definition per contract, with every path running through it.
//!
//! Each of these counts occurrences rather than listing call sites, so a
//! path added later is covered the moment it is written. A second
//! definition is how two paths that must agree start to drift, and the
//! count is what catches one.

use crate::render::{FILTER_BOXES_SCRIPT, SEARCH_OVERLAY_SCRIPT};

#[test]
fn every_key_handler_declines_a_composition() {
    // Mid-composition a key belongs to the IME: Enter accepts its
    // candidate and Escape abandons it. A handler that acted on one
    // anyway would dismiss the box or the dialog the reader was still
    // typing into and commit the half-composed text behind it. So every
    // `keydown` listener in the control scripts declines one, counted
    // rather than listed so a listener added later is covered the moment
    // it is written.
    for (name, script) in [
        ("filter_boxes.js", FILTER_BOXES_SCRIPT),
        ("search_overlay.js", SEARCH_OVERLAY_SCRIPT),
    ] {
        let listeners = script.matches(r#"addEventListener("keydown""#).count();
        assert!(listeners > 0, "{name} should listen for keydown at all");
        assert_eq!(
            script.matches("event.isComposing").count(),
            listeners,
            "each of {name}'s keydown listeners must decline a composition",
        );
    }
}

#[test]
fn every_way_out_of_a_filter_box_clears_it() {
    // Escape, the funnel and following one of the entries all dismiss a
    // box, and each has to clear it: a box that hid while its query still
    // narrowed the list would leave the reader with entries missing and
    // nothing on screen to say why. One `closeBox` does that for them all
    // — clearing by hand on a second path is how they would drift — so
    // the count below is its definition plus the two paths that reach it,
    // `dismiss` and the entry-click handler.
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
