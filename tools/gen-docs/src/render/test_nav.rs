//! The collapsible navigation drawer: the hamburger, the sidebar it is
//! the adjacent sibling of, and the script that opens it.

use super::test_fixtures::{fake_context, fake_rule};
use super::{NAV_TOGGLE_SCRIPT, NAV_TOGGLE_SCRIPT_FILENAME, render_page};

#[test]
fn page_emits_nav_toggle_button_and_sibling_nav() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    // Toggle is a plain <button> driven by `aria-expanded`, not
    // a <summary>/<details>, so JS can control open state and
    // the CSS adjacent-sibling selector is keyed off the same
    // attribute. The button starts in the closed state and is
    // emitted with `hidden` so it appears only after the script
    // wires up its (entirely JS-driven) behaviour — otherwise a
    // CSP-blocked or otherwise non-executing script would leave
    // a visible-but-inert hamburger on the page.
    // The button must literally carry the `hidden` attribute —
    // not just have the word "hidden" somewhere on the page
    // (the CSS comments mention `[hidden]` repeatedly). Anchor
    // the assertion to the toggle's opening tag so a refactor
    // that drops `hidden` from the template can't slip past.
    assert!(html.contains(r#"<button class="nav-toggle" type="button" hidden "#));
    assert!(html.contains(r#"aria-controls="nav-sidebar""#));
    assert!(html.contains(r#"aria-expanded="false""#));
    assert!(html.contains(r#"aria-label="Toggle navigation""#));
    assert!(html.contains(r#"aria-label="Lint rules""#));
    // The narrow-viewport CSS uses
    // `.nav-toggle[aria-expanded="true"] + .nav-sidebar`, so
    // the <nav> must be the immediate next sibling of
    // </button>. A refactor that nested the <nav> inside the
    // button or inserted another element between them would
    // break the overlay on narrow viewports; pin the literal
    // boundary.
    assert!(
        html.contains(r#"</button><nav class="nav-sidebar" id="nav-sidebar""#),
        r#"expected </button> to be immediately followed by the <nav class="nav-sidebar"> sibling"#,
    );
    // The close (✕) button lives inside the overlay so the
    // drawer can be dismissed without any fixed-position
    // element acting as both opener and closer. It must also
    // come *before* the title in DOM order: the hamburger
    // sits at the top-left, so on close the user's finger is
    // already there — putting the ✕ next to a tappable rule
    // link would invite a misclick on close.
    let header_start = html
        .find(r#"<div class="nav-sidebar-header">"#)
        .expect("nav-sidebar-header missing");
    let close_pos = html[header_start..]
        .find(r#"<button class="nav-sidebar-close""#)
        .expect("nav-sidebar-close missing");
    let title_pos = html[header_start..]
        .find(r#"<a class="nav-sidebar-title""#)
        .expect("nav-sidebar-title missing");
    assert!(
        close_pos < title_pos,
        "close button must appear before title in DOM order",
    );
}

#[test]
fn page_links_nav_toggle_script_externally() {
    let html = render_page(&[fake_rule("only")], &fake_context());
    // The script ships as a sibling file and is loaded via
    // `<script src>`, not inlined. Pin the exact tag so a
    // refactor that re-inlines the body (or renames the file
    // out of sync with `NAV_TOGGLE_SCRIPT_FILENAME`) is caught.
    assert!(
        html.contains(&format!(
            r#"<script src="{NAV_TOGGLE_SCRIPT_FILENAME}"></script>"#
        )),
        "expected the nav script to be referenced via <script src>",
    );
    // And the body must not be inlined any more: a `<script>`
    // with content would carry the IIFE opener verbatim.
    assert!(
        !html.contains("<script>(function () {"),
        "the nav script must not be inlined into the page",
    );
    // The page no longer ships a `<noscript>` rule: the toggle
    // is rendered with the HTML `hidden` attribute and the
    // script clears it once it wires up handlers, which covers
    // every "JS isn't running" mode — `<noscript>` only
    // covers "scripting disabled in browser" and isn't needed.
    // The JS file's own comments mention the term "noscript",
    // so we check for the closing tag (which only appears in
    // the actual element, never in a free-text comment).
    assert!(!html.contains("</noscript>"));
}

#[test]
fn nav_toggle_script_is_a_single_iife() {
    // Structural sanity check on `nav_toggle.js`. The whole
    // file is a single IIFE — every helper, every event
    // handler, every `var` is inside it. A duplicate-body bug
    // slipped past an earlier edit (commit f87b81d) by leaving
    // a stray `var ...; toggle.addEventListener(...); })();`
    // block AFTER the IIFE's closing line, producing a runtime
    // `ReferenceError: toggle is not defined` because those
    // `var`s were function-scoped to the now-closed IIFE.
    // Counting the IIFE opener and closer pins this shape so
    // a future edit can't reintroduce the same bug silently.
    assert_eq!(NAV_TOGGLE_SCRIPT.matches("(function () {").count(), 1);
    assert_eq!(NAV_TOGGLE_SCRIPT.matches("})();").count(), 1);
}
