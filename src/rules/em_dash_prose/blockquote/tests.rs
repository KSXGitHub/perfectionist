use super::scan_block_quotations;

/// The quoted text of every blockquote [`scan_block_quotations`] finds
/// in `text`.
fn quotations(text: &str) -> Vec<&str> {
    scan_block_quotations(text)
        .into_iter()
        .map(|range| &text[range])
        .collect()
}

/// A run of `>` lines is one block, and a blank line ends it.
#[test]
fn a_run_of_quote_lines_is_one_block() {
    let text = "intro\n\n> first — line\n> second line\n\nback to prose — here.\n";
    assert_eq!(quotations(text), ["> first — line\n> second line\n"]);
}

/// A `>` line holding nothing else keeps the block open, which is how
/// a quotation of several paragraphs is written.
#[test]
fn a_bare_marker_line_keeps_the_block_open() {
    let text = "> first para\n>\n> second para\n\nafter.\n";
    assert_eq!(quotations(text), ["> first para\n>\n> second para\n"]);
}

/// Markdown reads a `>`-less line after a quoted line as part of the
/// quotation, so the block covers it.
#[test]
fn a_lazy_continuation_line_stays_in_the_block() {
    let text = "> quoted line\ncontinued here\n\nafter.\n";
    assert_eq!(quotations(text), ["> quoted line\ncontinued here\n"]);
}

/// An alert borrows the blockquote syntax for an aside in the author's
/// own voice, so it is not reported as a quotation.
#[test]
fn an_alert_is_not_a_quotation() {
    for alert in [
        "> [!NOTE]\n> a remark — to the reader\n",
        "> [!TIP]\n> a remark — to the reader\n",
        "> [!IMPORTANT]\n> a remark — to the reader\n",
        "> [!WARNING]\n> a remark — to the reader\n",
        "> [!CAUTION]\n> a remark — to the reader\n",
    ] {
        assert_eq!(quotations(alert), [] as [&str; 0], "{alert:?}");
    }
}

/// Only the opening line decides: a bracketed-bang name further down
/// is quoted text like the rest of the block.
#[test]
fn a_bracketed_name_below_the_opening_line_is_quoted_text() {
    let text = "> a quotation\n> [!NOTE]\n";
    assert_eq!(quotations(text), ["> a quotation\n> [!NOTE]\n"]);
}

/// A bracketed name that is not the alert shape leaves the block a
/// quotation: a reference link is ordinary quoted markdown.
#[test]
fn a_reference_link_does_not_open_an_alert() {
    let text = "> [a link][target] — quoted\n";
    assert_eq!(quotations(text), ["> [a link][target] — quoted\n"]);
}

/// A text with no `>` line has no blockquote in it.
#[test]
fn plain_prose_holds_no_quotation() {
    assert_eq!(quotations("one — two\nthree\n"), [] as [&str; 0]);
}

/// Four spaces before the `>` make an indented code block rather than
/// a quotation, so the line opens no block here.
#[test]
fn a_deeply_indented_marker_opens_no_block() {
    assert_eq!(quotations("    > not a quotation\n"), [] as [&str; 0]);
    assert_eq!(quotations("   > a quotation\n"), ["   > a quotation\n"]);
}
