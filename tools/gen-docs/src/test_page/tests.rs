//! What the JavaScript test page has to hold to.
//!
//! Its lists of scripts and assets are written by hand, while everything
//! they name moves on its own: the catalogue renames a sheet, a case
//! file joins the directory beside this crate. These check that the two
//! still agree.

use super::{
    BASE_STYLESHEET_FILENAME, TEST_CASE_SCRIPTS, TEST_HARNESS_SCRIPT, TEST_REPORT_SCRIPT,
    render_test_page, test_page_assets, test_page_scripts,
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
fn every_case_file_is_on_the_page() {
    // A case file added to the directory is picked up by the
    // headless runner on its own, because that globs the directory.
    // This page cannot glob anything, so without this the two
    // runners would quietly disagree about what the suite is — and
    // the browser, the one that covers the older engines, would be
    // the side running less.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let mut on_disk: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {dir}: {error}"))
        .map(|entry| entry.expect("failed to read a directory entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".test.js"))
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = TEST_CASE_SCRIPTS
        .iter()
        .map(|&(name, _)| name.to_owned())
        .collect();
    listed.sort();
    assert_eq!(
        on_disk, listed,
        "every *.test.js in {dir} must be listed in TEST_CASE_SCRIPTS",
    );
    assert!(!listed.is_empty(), "the suite cannot be empty");
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
}

#[test]
fn the_page_asks_not_to_be_indexed() {
    assert!(render_test_page().contains(r#"<meta name="robots" content="noindex">"#));
}
