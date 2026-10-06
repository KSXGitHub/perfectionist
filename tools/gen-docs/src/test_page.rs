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

/// Every script the page loads, in load order. The two libraries under
/// test are named rather than carried: the catalogue already ships
/// them.
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
