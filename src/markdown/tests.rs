use super::*;

#[test]
fn empty_input_has_no_skip_ranges() {
    assert!(scan_skip_regions("").is_empty());
}

#[test]
fn plain_text_has_no_skip_ranges() {
    assert!(scan_skip_regions("hello world").is_empty());
}

#[test]
fn code_span_is_skip_region() {
    let text = "a `byte c` d";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    assert_eq!(&text[skips[0].clone()], "`byte c`");
}

#[test]
fn unmatched_backtick_is_not_a_skip() {
    let text = "a ` byte c d";
    let skips = scan_skip_regions(text);
    assert!(skips.is_empty());
}

#[test]
fn matching_backtick_runs_close_the_span() {
    let text = "a ``byte`c`` d";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    assert_eq!(&text[skips[0].clone()], "``byte`c``");
}

#[test]
fn autolink_is_skip() {
    let text = "see <https://example.com> please";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    assert_eq!(&text[skips[0].clone()], "<https://example.com>");
}

#[test]
fn html_like_tag_is_not_classified_as_autolink() {
    let text = "see <div> here";
    let skips = scan_skip_regions(text);
    assert!(skips.is_empty());
}

#[test]
fn inline_link_is_skip() {
    let text = "see [click](https://example.com) for more";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    assert_eq!(&text[skips[0].clone()], "[click](https://example.com)");
}

#[test]
fn reference_definition_is_skip() {
    let text = "[id]: https://example.com\nrest";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    assert!(text[skips[0].clone()].starts_with("[id]: https://example.com"));
}

#[test]
fn fenced_code_block_is_skip() {
    let text = "before\n```\ncode https://example.com\n```\nafter";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    let region = &text[skips[0].clone()];
    assert!(region.starts_with("```"));
    assert!(region.contains("https://example.com"));
}

#[test]
fn tab_indented_code_block_is_skip() {
    // A single leading tab is 4 columns (CommonMark §2.2), so the
    // line is an indented code block and its content is skipped.
    let text = "intro\n\n\tcode #123 here\n";
    let skips = scan_skip_regions(text);
    assert_eq!(skips.len(), 1);
    assert!(text[skips[0].clone()].contains("#123"));
}

#[test]
fn code_regions_mask_a_code_span_when_requested() {
    let text = "a `byte c` d";
    let with = scan_code_regions(text, true);
    assert_eq!(with.len(), 1);
    assert_eq!(&text[with[0].clone()], "`byte c`");
    // With code spans excluded from the mask, the span body is
    // left open to the prose scan, so no range is produced.
    assert!(scan_code_regions(text, false).is_empty());
}

#[test]
fn code_regions_always_mask_a_fenced_block() {
    let text = "before\n```\nlet x = \"…\";\n```\nafter";
    // The fence is masked regardless of the code-span knob.
    for include_code_spans in [true, false] {
        let skips = scan_code_regions(text, include_code_spans);
        assert_eq!(skips.len(), 1);
        assert!(text[skips[0].clone()].starts_with("```"));
        assert!(text[skips[0].clone()].contains('…'));
    }
}

#[test]
fn code_regions_ignore_links_and_autolinks() {
    // Unlike Tier A's `scan_skip_regions`, the code-region mask
    // leaves links and autolinks open to the prose scan.
    let text = "see [click](https://example.com) and <https://example.org>";
    assert!(scan_code_regions(text, true).is_empty());
}

#[test]
fn position_in_skip_is_inclusive_on_start_exclusive_on_end() {
    let skips = [Range { start: 5, end: 10 }];
    assert!(!position_in_skip(&skips, 4));
    assert!(position_in_skip(&skips, 5));
    assert!(position_in_skip(&skips, 9));
    assert!(!position_in_skip(&skips, 10));
}
