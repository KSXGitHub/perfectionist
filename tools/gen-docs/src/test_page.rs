//! Render the browser runner for the catalogue's JavaScript unit tests,
//! into the same output directory as the catalogue, so that whatever
//! deploys the one deploys the other.
//!
//! Why it ships rather than waiting in a checkout: the whole point of a
//! browser runner is the engines a headless one cannot reach, and
//! reaching one of those means opening a URL on it. A page whose reader
//! must clone the repository first is the one form that does not serve
//! its own purpose.
//!
//! The libraries it exercises are the catalogue's own files, loaded from
//! where the catalogue writes them rather than copied in, so what the
//! page reports is what the catalogue is running.

use crate::render::{
    MATCH_ADMIT_SCRIPT_FILENAME, MATCH_SCORE_SCRIPT_FILENAME, MATCH_SCRIPT_FILENAME,
    MATCH_TEXT_SCRIPT_FILENAME, MATCH_TIERS_SCRIPT_FILENAME, RANK_SCRIPT_FILENAME,
};
use maud::{DOCTYPE, Markup, html};

/// File name the rendered page is written under, beside the catalogue's
/// `index.html`.
pub(crate) const TEST_PAGE_FILENAME: &str = "tests.html";

/// The page's own stylesheet, written beside it and linked after
/// [`BASE_STYLESHEET_FILENAME`].
pub(crate) const TEST_PAGE_CSS: &str = include_str!("style/tests.css");

/// File name [`TEST_PAGE_CSS`] is written under; the page's
/// `<link rel="stylesheet">` references the same name, so they must
/// agree.
pub(crate) const TEST_PAGE_CSS_FILENAME: &str = "tests.css";

/// The catalogue page this one links back to, so a reader who arrives
/// at this URL has somewhere to go.
const CATALOGUE_FILENAME: &str = "index.html";

/// The catalogue sheet this page borrows its font, body width and
/// `[hidden]` reset from. It is the first entry of
/// `crate::render::STYLESHEETS`, and a test below holds it to that
/// rather than letting the name drift.
const BASE_STYLESHEET_FILENAME: &str = "base.css";

/// The registry and assertions the cases register against. Loaded
/// before any of them, since each calls `add` as it is evaluated.
pub(crate) const TEST_HARNESS_SCRIPT: (&str, &str) =
    ("harness.js", include_str!("../tests/harness.js"));

/// The fixtures the cases are built from. Loaded after the harness and
/// before any case, since a case reads them as it is evaluated. Held to
/// the directory by the same test as the cases below.
pub(crate) const TEST_FIXTURE_SCRIPTS: &[(&str, &str)] = &[(
    "rank.fixtures.js",
    include_str!("../tests/rank.fixtures.js"),
)];

/// The cases, in the order the page runs them. Every `*.test.js` in
/// `tools/gen-docs/tests/` belongs here, and a test below reads that
/// directory and fails if one is missing, so the page and the headless
/// runner (which globs the same directory) can never disagree about what
/// the suite is.
pub(crate) const TEST_CASE_SCRIPTS: &[(&str, &str)] = &[
    (
        "match_excerpt.test.js",
        include_str!("../tests/match_excerpt.test.js"),
    ),
    (
        "match_folding.test.js",
        include_str!("../tests/match_folding.test.js"),
    ),
    (
        "match_ordering.test.js",
        include_str!("../tests/match_ordering.test.js"),
    ),
    (
        "match_ranges.test.js",
        include_str!("../tests/match_ranges.test.js"),
    ),
    (
        "match_reordered.test.js",
        include_str!("../tests/match_reordered.test.js"),
    ),
    (
        "match_respaced.test.js",
        include_str!("../tests/match_respaced.test.js"),
    ),
    (
        "match_variants.test.js",
        include_str!("../tests/match_variants.test.js"),
    ),
    (
        "match_worth_showing.test.js",
        include_str!("../tests/match_worth_showing.test.js"),
    ),
    (
        "rank_finding.test.js",
        include_str!("../tests/rank_finding.test.js"),
    ),
    (
        "rank_ordering.test.js",
        include_str!("../tests/rank_ordering.test.js"),
    ),
    (
        "rank_result_text.test.js",
        include_str!("../tests/rank_result_text.test.js"),
    ),
    (
        "rank_text.test.js",
        include_str!("../tests/rank_text.test.js"),
    ),
];

/// The reporter, which runs the registered cases and writes the result
/// into the page. Loaded last, after every case has registered.
pub(crate) const TEST_REPORT_SCRIPT: (&str, &str) =
    ("report.js", include_str!("../tests/report.js"));

/// The libraries under test, in the order the page loads them. Named
/// rather than carried: the catalogue already ships them beside this
/// page. `tools/gen-docs/tests/run.mjs` names the same files in the same
/// order, which `the_two_runners_load_the_same_libraries` holds it to.
pub(crate) const TEST_LIBRARIES: &[&str] = &[
    MATCH_TEXT_SCRIPT_FILENAME,
    MATCH_SCORE_SCRIPT_FILENAME,
    MATCH_ADMIT_SCRIPT_FILENAME,
    MATCH_TIERS_SCRIPT_FILENAME,
    MATCH_SCRIPT_FILENAME,
    RANK_SCRIPT_FILENAME,
];

/// Every script the page loads, in load order.
pub(crate) fn test_page_scripts() -> Vec<&'static str> {
    let mut scripts = TEST_LIBRARIES.to_vec();
    scripts.push(TEST_HARNESS_SCRIPT.0);
    scripts.extend(TEST_FIXTURE_SCRIPTS.iter().map(|&(name, _)| name));
    scripts.extend(TEST_CASE_SCRIPTS.iter().map(|&(name, _)| name));
    scripts.push(TEST_REPORT_SCRIPT.0);
    scripts
}

/// Every file this page needs written beside it that the catalogue does
/// not already ship, as `(filename, contents)`.
pub(crate) fn test_page_assets() -> Vec<(&'static str, &'static str)> {
    let mut assets = vec![(TEST_PAGE_CSS_FILENAME, TEST_PAGE_CSS), TEST_HARNESS_SCRIPT];
    assets.extend(TEST_FIXTURE_SCRIPTS.iter().copied());
    assets.extend(TEST_CASE_SCRIPTS.iter().copied());
    assets.push(TEST_REPORT_SCRIPT);
    assets
}

pub(crate) fn render_test_page() -> String {
    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="description" content="Unit tests for the perfectionist lint catalogue's own browser scripts.";
                // Keeps the page out of search results: it is a
                // contributor's tool that happens to sit on a public
                // site, and a reader searching for a lint should not
                // land on a list of assertions.
                meta name="robots" content="noindex";
                title { "perfectionist docs-site JS tests" }
                link rel="stylesheet" href=(BASE_STYLESHEET_FILENAME);
                link rel="stylesheet" href=(TEST_PAGE_CSS_FILENAME);
                @for src in test_page_scripts() {
                    link rel="preload" as="script" href=(src);
                }
            }
            body {
                h1 { "docs-site JS tests" }
                p {
                    "The "
                    a href=(CATALOGUE_FILENAME) { "perfectionist lint catalogue" }
                    "'s own JavaScript, tested in this browser."
                }
                p id="summary" {}
                p id="engine" {}
                // How many case files the page loaded, which report.js
                // has no other way of knowing: see its own note on what
                // a file an engine rejects leaves behind.
                div id="cases" data-expected-groups=(TEST_CASE_SCRIPTS.len()) {}
                @for src in test_page_scripts() {
                    script src=(src) {}
                }
            }
        }
    };
    markup.into_string()
}

#[cfg(test)]
mod tests;
