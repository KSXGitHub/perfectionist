//! The contracts the browser scripts keep. The groups below divide them
//! by the kind of claim each makes rather than by which affordance it
//! covers, because one claim usually spans both: the overlay and the
//! filter boxes share their libraries, their markup contract and the
//! single definition each way in runs through.
//!
//! These read the scripts as source text. A script that crossed any of
//! these lines would still drive a working page, so running it proves
//! nothing and only reading it will do.

mod libraries;
mod loading;
mod one_definition;
mod provided;

use crate::render::{
    FILTER_BOXES_SCRIPT, FILTER_BOXES_SCRIPT_FILENAME, HIGHLIGHT_SCRIPT, HIGHLIGHT_SCRIPT_FILENAME,
    MATCH_ADMIT_SCRIPT, MATCH_ADMIT_SCRIPT_FILENAME, MATCH_SCORE_SCRIPT,
    MATCH_SCORE_SCRIPT_FILENAME, MATCH_SCRIPT, MATCH_SCRIPT_FILENAME, MATCH_TEXT_SCRIPT,
    MATCH_TEXT_SCRIPT_FILENAME, MATCH_TIERS_SCRIPT, MATCH_TIERS_SCRIPT_FILENAME, RANK_SCRIPT,
    RANK_SCRIPT_FILENAME, SEARCH_OVERLAY_SCRIPT, SEARCH_OVERLAY_SCRIPT_FILENAME,
};

/// Every file the search is built from: the name it ships under, its
/// source, and the one global it declares, where it declares one. The ones
/// that declare a global are the libraries; the ones that don't are the
/// controls, and each of those reads at least one library.
const SEARCH_SCRIPTS: [(&str, &str, Option<&str>); 9] = [
    (
        MATCH_TEXT_SCRIPT_FILENAME,
        MATCH_TEXT_SCRIPT,
        Some("perfectionistMatchText"),
    ),
    (
        MATCH_SCORE_SCRIPT_FILENAME,
        MATCH_SCORE_SCRIPT,
        Some("perfectionistMatchScore"),
    ),
    (
        MATCH_ADMIT_SCRIPT_FILENAME,
        MATCH_ADMIT_SCRIPT,
        Some("perfectionistMatchAdmit"),
    ),
    (
        MATCH_TIERS_SCRIPT_FILENAME,
        MATCH_TIERS_SCRIPT,
        Some("perfectionistMatchTiers"),
    ),
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
