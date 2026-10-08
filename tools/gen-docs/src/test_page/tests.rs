//! What the JavaScript test page has to hold to.
//!
//! Its lists of scripts and assets are written by hand, while everything
//! they name moves on its own: the catalogue renames a sheet, a case
//! file joins the directory beside this crate. These check that the two
//! still agree.

use super::{
    BASE_STYLESHEET_FILENAME, TEST_CASE_SCRIPTS, TEST_FIXTURE_SCRIPTS, TEST_HARNESS_SCRIPT,
    TEST_LIBRARIES, TEST_PAGE_CSS, TEST_REPORT_SCRIPT, render_test_page, test_page_assets,
    test_page_scripts,
};
use crate::render::STYLESHEETS;
use std::fs;

#[test]
fn base_stylesheet_is_the_first_catalogue_sheet() {
    // The page borrows one sheet from the catalogue by name, so the
    // name has to be one the catalogue actually writes out.
    assert_eq!(STYLESHEETS[0].0, BASE_STYLESHEET_FILENAME);
}

#[test]
fn every_case_and_fixture_file_is_on_the_page() {
    // A file added to the directory is picked up by the headless runner
    // on its own, because that globs the directory. This page cannot
    // glob anything, so without this the two runners would quietly
    // disagree about what the suite is — and the browser, the one that
    // covers the older engines, would be the side running less.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let names = |suffix: &str| {
        let mut found: Vec<String> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("failed to read {dir}: {error}"))
            .map(|entry| entry.expect("failed to read a directory entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| name.ends_with(suffix))
            .collect();
        found.sort();
        found
    };
    // Left unsorted, against a sorted directory listing, so the lists are
    // held to the order the headless runner reads the same files in —
    // which it gets from `readdir().sort()`. Membership alone would let
    // one fixture load after another that reads it in one runner and
    // before it in the other.
    let listed = |scripts: &[(&str, &str)]| {
        scripts
            .iter()
            .map(|&(name, _)| name.to_owned())
            .collect::<Vec<String>>()
    };
    // `.test.js` ends with `.js`, so the fixtures are looked for by the
    // longer suffix and the cases excluded from what that finds.
    assert_eq!(
        names(".test.js"),
        listed(TEST_CASE_SCRIPTS),
        "every *.test.js in {dir} must be listed in TEST_CASE_SCRIPTS, in that order",
    );
    assert_eq!(
        names(".fixtures.js"),
        listed(TEST_FIXTURE_SCRIPTS),
        "every *.fixtures.js in {dir} must be listed in TEST_FIXTURE_SCRIPTS, in that order",
    );
    assert!(!TEST_CASE_SCRIPTS.is_empty(), "the suite cannot be empty");
}

#[test]
fn the_harness_loads_before_the_cases_and_the_reporter_after_them() {
    // Each case calls the harness's `add` as it is evaluated, and
    // the reporter runs whatever has registered by the time it runs,
    // so this order is what makes the page report anything at all.
    let scripts = test_page_scripts();
    let position = |name: &str| {
        scripts
            .iter()
            .position(|&script| script == name)
            .unwrap_or_else(|| panic!("{name} is not loaded by the page"))
    };
    let harness = position(TEST_HARNESS_SCRIPT.0);
    let reporter = position(TEST_REPORT_SCRIPT.0);
    for &(name, _) in TEST_CASE_SCRIPTS {
        assert!(harness < position(name), "harness.js must precede {name}");
        assert!(position(name) < reporter, "{name} must precede report.js");
    }
    // A fixture is read by the cases built on it as they are evaluated,
    // and reads the harness itself, so it sits between the two.
    for &(name, _) in TEST_FIXTURE_SCRIPTS {
        assert!(harness < position(name), "harness.js must precede {name}");
        for &(case, _) in TEST_CASE_SCRIPTS {
            assert!(
                position(name) < position(case),
                "{name} must precede {case}",
            );
        }
    }
}

#[test]
fn the_page_loads_the_libraries_it_tests_and_every_asset_it_ships() {
    let html = render_test_page();
    for src in test_page_scripts() {
        assert!(
            html.contains(&format!(r#"<script src="{src}"></script>"#)),
            "expected {src} to be loaded via <script src>",
        );
    }
    // Every file written beside the page is one the page asks for —
    // an asset nothing references is dead weight on the deploy.
    for (name, _) in test_page_assets() {
        assert!(
            html.contains(&format!(r#""{name}""#)),
            "expected {name} to be referenced by the page",
        );
    }
    // The two libraries are the catalogue's own files, loaded from
    // where the catalogue puts them rather than copied in.
    assert!(html.contains(r#"<script src="match.js"></script>"#));
    assert!(html.contains(r#"<script src="rank.js"></script>"#));
}

#[test]
fn every_case_file_opens_a_group_named_after_itself() {
    // Both runners head a file's cases with the name it opened its group
    // under, so a file copied from its neighbour and edited would file
    // its cases under the neighbour's heading, in both runners at once.
    for (name, source) in TEST_CASE_SCRIPTS {
        let declaration = format!(r#"group("{name}")"#);
        assert!(
            source.contains(&declaration),
            "{name} should open its group with {declaration}",
        );
    }
}

#[test]
fn the_page_carries_every_id_the_reporter_looks_up() {
    // report.js finds its elements by id and bails when one is missing,
    // which would leave the page blank with nothing on it to say why. The
    // ids come out of its source rather than a list here, so one added
    // later is covered with no edit.
    let html = render_test_page();
    let opener = r##"querySelector("#"##;
    let mut looked_up = 0;
    let mut rest = TEST_REPORT_SCRIPT.1;
    while let Some(at) = rest.find(opener) {
        rest = &rest[at + opener.len()..];
        let id = rest
            .split('"')
            .next()
            .expect("unterminated querySelector argument");
        assert!(
            html.contains(&format!(r#"id="{id}""#)),
            "report.js looks up #{id}, which the page does not carry",
        );
        looked_up += 1;
    }
    assert!(
        looked_up > 0,
        "no id lookups found in report.js, so the scan above checked nothing",
    );
    // The scan knows one way of looking an element up, so every way the
    // reporter reaches the document has to be that one or a `createElement`.
    // Without this it could look an element up by some other call, go
    // unscanned, and still leave a count above zero here.
    let source = TEST_REPORT_SCRIPT.1;
    assert_eq!(
        source.matches("document.").count(),
        looked_up + source.matches("document.createElement(").count(),
        "report.js should reach the document only to look an id up the one way \
         this scans for, or to build an element, or the scan above misses a lookup",
    );
}

#[test]
fn every_state_the_reporter_sets_is_styled() {
    // A row's state is carried by an attribute, a glyph and an accessible
    // name, and tests.css colours it by that attribute. A state added to
    // the reporter's table without a rule beside it would render in
    // whatever colour it inherited, saying nothing. The states are read
    // out of the table rather than listed here, so a fourth is covered
    // the moment it is written.
    let table = TEST_REPORT_SCRIPT
        .1
        .split_once("var MARKS = {")
        .expect("report.js should declare a MARKS table")
        .1
        .split_once("};")
        .expect("report.js's MARKS table should be terminated")
        .0;
    // Split on the separator between entries rather than on newlines: a
    // table written on one line would otherwise yield one "state" — the
    // first — and the count below would still pass while the rest went
    // unchecked.
    let mut styled = 0;
    for entry in table.split(',') {
        let Some((name, _)) = entry.trim().split_once(':') else {
            continue;
        };
        assert!(
            TEST_PAGE_CSS.contains(&format!(r#"[data-result="{name}"]"#)),
            "the reporter can set the state `{name}`, which tests.css does not style",
        );
        styled += 1;
    }
    assert!(styled > 0, "no states found in the reporter's MARKS table");
}

#[test]
fn the_page_says_how_many_case_files_it_loaded() {
    // report.js reports whatever registered, and a case file an engine
    // rejects outright registers nothing — the one failure this page
    // exists to find. The count is the page's to supply, so a reporter
    // reading it back can tell a shrunken suite from a passing one.
    assert!(
        render_test_page().contains(&format!(
            r#"data-expected-groups="{}""#,
            TEST_CASE_SCRIPTS.len()
        )),
        "the page must carry one expected group per case file",
    );
    assert!(
        TEST_REPORT_SCRIPT
            .1
            .contains(r#"getAttribute("data-expected-groups")"#),
        "report.js must read the count the page carries",
    );
}

#[test]
fn the_page_asks_not_to_be_indexed() {
    assert!(render_test_page().contains(r#"<meta name="robots" content="noindex">"#));
}

#[test]
fn the_two_runners_load_the_same_libraries() {
    // The terminal runner globs the case files but cannot read Rust, so
    // it names the libraries itself. If the two lists drift, one runner
    // exercises code the other does not, and the browser — the only one
    // that says anything about the engines the catalogue targets — is the
    // one that would quietly fall behind.
    let runner = include_str!("../../tests/run.mjs");
    let list = runner
        .split_once("const libraries = [")
        .expect("run.mjs must declare its libraries in one array")
        .1
        .split_once(']')
        .expect("`const libraries = [` must be closed")
        .0;
    let named: Vec<&str> = list
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"'))
        .filter_map(|line| line.split_once('"'))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        named, TEST_LIBRARIES,
        "run.mjs and the test page must load the same libraries in the same order",
    );
}
