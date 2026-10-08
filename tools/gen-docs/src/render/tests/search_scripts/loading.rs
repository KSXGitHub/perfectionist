//! How the page loads the scripts: each as its own file, all of them
//! written beside it, in an order where every library has run before
//! whatever reads its global, and each wrapped so it publishes that
//! global and nothing else.

use super::{SEARCH_SCRIPTS, strip_js_comments};
use crate::render::tests::{fake_context, fake_rule};
use crate::render::{PAGE_SCRIPT_FILES, PAGE_SCRIPTS, render_page};

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
fn page_scripts_are_the_files_that_ship() {
    // One list drives the `<script src>` tags and the preload hints, the
    // other drives what is written beside `index.html`. A script in the
    // first and not the second is a 404 on every page load; one in the
    // second and not the first ships unread.
    let shipped: Vec<&str> = PAGE_SCRIPT_FILES.iter().map(|&(name, _)| name).collect();
    assert_eq!(
        shipped, PAGE_SCRIPTS,
        "every page script must be written beside the page, in load order",
    );
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
