//! Render `tests.html`: the browser runner for the catalogue's
//! JavaScript unit tests, written into the same output directory as the
//! catalogue and deployed with it.
//!
//! The cases also run headlessly, and that is the run CI gates on. This
//! page is what makes them runnable in an engine no headless runner here
//! reaches: the scripts ship to browsers years older than the Node the
//! cases run under, and nothing on that side would catch a construct V8
//! accepts today and an older engine rejects outright. Shipped with the
//! site, it is a URL to open on whatever device is in question rather
//! than a file whose reader must first clone the repository.
//!
//! It loads the same `match.js` and `rank.js` the catalogue loads — the
//! shipped files sitting beside it, not copies of them — so what it
//! reports is what the catalogue is running.
//!
//! Nothing links to it. It is a contributor's page on a consumer's site,
//! and a reader looking for a lint has no use for a list of assertions.

use crate::render::{MATCH_SCRIPT_FILENAME, RANK_SCRIPT_FILENAME};
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

/// The catalogue sheet this page borrows its font, body width and
/// `[hidden]` reset from. It is the first entry of
/// `crate::render::STYLESHEETS`, and a test below holds it to that
/// rather than letting the name drift.
const BASE_STYLESHEET_FILENAME: &str = "base.css";

/// The registry and assertions the cases register against. Loaded
/// before any of them, since each calls `add` as it is evaluated.
pub(crate) const TEST_HARNESS_SCRIPT: (&str, &str) =
    ("harness.js", include_str!("../tests/harness.js"));

/// The cases, in the order the page runs them. Every `*.test.js` in
/// `tools/gen-docs/tests/` belongs here, and a test below reads that
/// directory and fails if one is missing, so the page and the headless
/// runner (which globs the same directory) can never disagree about what
/// the suite is.
pub(crate) const TEST_CASE_SCRIPTS: &[(&str, &str)] = &[
    ("match.test.js", include_str!("../tests/match.test.js")),
    ("rank.test.js", include_str!("../tests/rank.test.js")),
];

/// The reporter, which runs the registered cases and writes the result
/// into the page. Loaded last, after every case has registered.
pub(crate) const TEST_REPORT_SCRIPT: (&str, &str) =
    ("report.js", include_str!("../tests/report.js"));

/// Every script the page loads, in load order: the two libraries under
/// test (already shipped for the catalogue, so only named here), the
/// harness, the cases, then the reporter.
pub(crate) fn test_page_scripts() -> Vec<&'static str> {
    let mut scripts = vec![
        MATCH_SCRIPT_FILENAME,
        RANK_SCRIPT_FILENAME,
        TEST_HARNESS_SCRIPT.0,
    ];
    scripts.extend(TEST_CASE_SCRIPTS.iter().map(|&(name, _)| name));
    scripts.push(TEST_REPORT_SCRIPT.0);
    scripts
}

/// Every file this page needs written beside it that the catalogue does
/// not already ship, as `(filename, contents)`.
pub(crate) fn test_page_assets() -> Vec<(&'static str, &'static str)> {
    let mut assets = vec![(TEST_PAGE_CSS_FILENAME, TEST_PAGE_CSS), TEST_HARNESS_SCRIPT];
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
                    "The same cases "
                    code { "just test-js" }
                    " runs headlessly, run here by this browser against the "
                    "catalogue's own "
                    code { "match.js" }
                    " and "
                    code { "rank.js" }
                    "."
                }
                p id="summary" {}
                ul id="cases" {}
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
