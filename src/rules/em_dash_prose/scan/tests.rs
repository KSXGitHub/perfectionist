use super::{Dash, DashPolicy, scan_dashes};
use crate::markdown::scan_code_regions;

/// Every dash `policy` reports in `text`, as the text each one spans
/// paired with what the scan called it.
fn scan(text: &str, policy: DashPolicy) -> Vec<(&str, Dash)> {
    let skips = scan_code_regions(text, true);
    scan_dashes(text, &skips, policy)
        .into_iter()
        .map(|(range, dash)| (&text[range], dash))
        .collect()
}

/// The policy with every knob on, which is the shipped default.
const ALL: DashPolicy = DashPolicy {
    en_dash: true,
    ascii_run: true,
};

#[test]
fn reports_the_unicode_dashes() {
    assert_eq!(
        scan("one — two – three", ALL),
        [("—", Dash::Em), ("–", Dash::En)],
    );
}

/// `flag_en_dash = false` keeps U+2013 out of the report and leaves
/// U+2014 in, the em dash having no knob of its own.
#[test]
fn en_dash_is_reported_only_when_the_policy_asks() {
    let policy = DashPolicy {
        en_dash: false,
        ..ALL
    };
    assert_eq!(scan("one — two – three", policy), [("—", Dash::Em)]);
}

/// A Unicode dash is reported wherever it falls, no space required on
/// either side, so the en dash a numeric range is written with is
/// reported where the ASCII run spelling of that range is not.
#[test]
fn a_numeric_range_written_with_an_en_dash_is_reported() {
    assert_eq!(scan("10–20", ALL), [("–", Dash::En)]);
    assert_eq!(scan("10--20", ALL), []);
}

/// A hyphen run counts only with a space on each side, which is what
/// separates a dash stand-in from the runs that carry their own
/// meaning.
#[test]
fn reports_a_spaced_hyphen_run_only() {
    assert_eq!(scan("one -- two", ALL), [("--", Dash::AsciiRun)]);
    assert_eq!(scan("one --- two", ALL), [("---", Dash::AsciiRun)]);
    for unspaced in [
        "pass --help to it",
        "the --version flag",
        "a <!-- comment --> here",
        "an arrow a --> b",
        "a double-barrelled--word",
        "trailing dashes --",
        "-- leading dashes",
    ] {
        assert_eq!(scan(unspaced, ALL), [], "{unspaced:?}");
    }
}

/// A lone hyphen is a hyphen, so neither a hyphenated word nor a
/// spaced minus sign is a dash stand-in.
#[test]
fn a_single_hyphen_is_never_a_dash() {
    assert_eq!(scan("a - b", ALL), []);
    assert_eq!(scan("well-known", ALL), []);
}

/// `flag_pseudo_em_dash = false` leaves a spaced hyphen run alone and
/// still reports the Unicode dashes beside it.
#[test]
fn hyphen_runs_are_reported_only_when_the_policy_asks() {
    let policy = DashPolicy {
        ascii_run: false,
        ..ALL
    };
    assert_eq!(scan("one -- two — three", policy), [("—", Dash::Em)]);
}

/// An inline code span holds code rather than a sentence, so the
/// argument separator in a quoted shell command stays.
#[test]
fn a_code_span_keeps_its_dashes() {
    assert_eq!(scan("run `cargo dylint -- --all-targets` now", ALL), []);
    assert_eq!(scan("the glyph `—` itself", ALL), []);
}

/// A fenced block tagged for another language keeps its dashes too:
/// `--` opens a comment in Lua, and rewriting the sentence around it
/// would be nonsense.
#[test]
fn a_fenced_code_block_keeps_its_dashes() {
    let text = "Example:\n\n```lua\nlocal x = 1 -- a comment\n```\n\nback to prose.";
    assert_eq!(scan(text, ALL), []);
}

/// A four-space-indented block is a code block as much as a fenced
/// one, which is where a doc comment's untagged examples live.
#[test]
fn an_indented_code_block_keeps_its_dashes() {
    let text = "Example:\n\n    local x = 1 -- a comment\n\nback to prose.";
    assert_eq!(scan(text, ALL), []);
}

/// A run split across two joined comment lines has a newline on one
/// side, so it is not reported and no span has to cross the join.
#[test]
fn a_newline_does_not_count_as_the_surrounding_space() {
    assert_eq!(scan("first line --\nsecond line", ALL), []);
    assert_eq!(scan("first line\n-- second line", ALL), []);
}

/// A quotation is someone else's words, so both dash spellings inside
/// one stay, whichever ASCII quote mark delimits it.
#[test]
fn an_ascii_quotation_keeps_its_dashes() {
    assert_eq!(scan(r#"he wrote "one — two" there"#, ALL), []);
    assert_eq!(scan("he wrote 'one — two' there", ALL), []);
    assert_eq!(scan(r#"he wrote "one -- two" there"#, ALL), []);
    assert_eq!(scan("he wrote 'one -- two' there", ALL), []);
}

/// A quote mark opening the text opens a quotation: a comment whose
/// rendered text starts with one has nothing before the mark to rule
/// it out.
#[test]
fn a_quotation_opening_the_text_keeps_its_dashes() {
    assert_eq!(scan(r#""one — two" and more"#, ALL), []);
    assert_eq!(scan("'one — two' and more", ALL), []);
}

/// The typographic quotation marks delimit a quotation as much as the
/// ASCII ones do, each pair closing with its own mark.
#[test]
fn a_typographic_quotation_keeps_its_dashes() {
    for quoted in [
        "he wrote ‘one — two’ there",
        "he wrote “one — two” there",
        "he wrote ‚one — two‘ there",
        "he wrote „one — two“ there",
        "he wrote «one — two» there",
        "he wrote ‹one — two› there",
        "he wrote 「one — two」 there",
        "he wrote 『one — two』 there",
        "he wrote “one -- two” there",
    ] {
        assert_eq!(scan(quoted, ALL), [], "{quoted:?}");
    }
}

/// A Chinese title mark delimits the name of a work, which is
/// reproduced as verbatim as a quotation, so a dash inside one stays.
/// The single marks carry a title too: a style guide may use them for
/// a title nested in another or for one standing on its own.
#[test]
fn a_title_mark_keeps_its_dashes() {
    for titled in [
        "the book 《one — two》 is out",
        "the essay 〈one — two〉 is out",
        "the book 《one -- two》 is out",
    ] {
        assert_eq!(scan(titled, ALL), [], "{titled:?}");
    }
}

/// A single title mark inside a double one is swallowed by the outer
/// span, so the dash between the two stays.
#[test]
fn a_nested_title_mark_keeps_the_outer_span() {
    assert_eq!(scan("《a book 〈a chapter〉 — and more》", ALL), []);
}

/// Prose on either side of a quotation is still the author's own, so a
/// dash outside the quote marks is reported.
#[test]
fn prose_around_a_quotation_is_still_scanned() {
    assert_eq!(
        scan(r#"before — "quoted" — after"#, ALL),
        [("—", Dash::Em), ("—", Dash::Em)],
    );
}

/// An apostrophe has a letter on its left, so it opens no quotation
/// and a sentence full of them pairs up no span to mask.
#[test]
fn an_apostrophe_opens_no_quotation() {
    assert_eq!(scan("can't — won't", ALL), [("—", Dash::Em)]);
    assert_eq!(scan("the user's — not ours", ALL), [("—", Dash::Em)]);
    assert_eq!(scan("it's -- really", ALL), [("--", Dash::AsciiRun)]);
    assert_eq!(scan("the user’s own — not ours", ALL), [("—", Dash::Em)]);
}

/// A single quotation closes at a mark that ends the quoted word, so
/// an apostrophe inside the quotation does not close it early and
/// leave the rest of it read as prose.
#[test]
fn an_apostrophe_inside_a_single_quotation_does_not_close_it() {
    assert_eq!(scan("'he isn't — sure' now", ALL), []);
}

/// A single quotation closes at a mark the quoted text runs up to, so
/// a mark with a space before it does not close it early.
#[test]
fn a_mark_with_a_space_before_it_does_not_close_a_single_quotation() {
    assert_eq!(scan("'a ' b — c' d", ALL), []);
}

/// A single quote mark followed by a space quotes nothing, so it opens
/// no quotation and the dash after it is still the author's own.
#[test]
fn a_mark_followed_by_a_space_opens_no_single_quotation() {
    assert_eq!(scan("use ' a — b' here", ALL), [("—", Dash::Em)]);
}

/// A Rust lifetime reads as an opening quote and finds nothing to
/// close it, so it masks nothing after itself.
#[test]
fn a_lifetime_masks_nothing() {
    assert_eq!(
        scan("a 'tcx parameter — and a 'a one", ALL),
        [("—", Dash::Em)],
    );
}

/// An unpaired quote mark closes nothing, so the prose after it is
/// scanned as prose.
#[test]
fn an_unpaired_quote_mark_masks_nothing() {
    assert_eq!(scan(r#"the " character — alone"#, ALL), [("—", Dash::Em)]);
    assert_eq!(scan(r#"a 6" pipe — long"#, ALL), [("—", Dash::Em)]);
}

/// A quotation closes at the end of its line, so an opening quote mark
/// does not reach across a joined comment line to mask the next one.
/// A mark that would close it waits on the second line, so a scan
/// willing to cross the join would find one.
#[test]
fn a_quotation_does_not_cross_a_line() {
    assert_eq!(
        scan("an opening \" here\nand a dash — there \" close", ALL),
        [("—", Dash::Em)],
    );
    assert_eq!(
        scan("an opening 'word here\nand a dash — there word' close", ALL),
        [("—", Dash::Em)],
    );
}

/// A typographic quotation closes at the end of its line too, so an
/// opening mark does not reach across a joined comment line to mask
/// the next one. The mark that closes the pair waits on the second
/// line, so a scan willing to cross the join would find one.
#[test]
fn a_typographic_quotation_does_not_cross_a_line() {
    assert_eq!(
        scan("an opening “ here\nand a dash — there ” close", ALL),
        [("—", Dash::Em)],
    );
}

/// An apostrophe inside a code span pairs with nothing outside it: the
/// span is masked before the quotation scan runs, so the prose after
/// it is still scanned.
#[test]
fn an_apostrophe_in_a_code_span_masks_nothing_after_it() {
    assert_eq!(
        scan("the `don't` helper — see above", ALL),
        [("—", Dash::Em)],
    );
}
