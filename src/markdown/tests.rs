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

/// Classify `input` and return the `(matched-text, kind)` pairs, for
/// concise assertions in the classifier tests below.
fn classified(input: &str, options: ClassifyOptions) -> Vec<(&str, ConstructKind)> {
    classify_constructs(input, options)
        .into_iter()
        .map(|found| (&input[found.range.clone()], found.kind))
        .collect()
}

#[test]
fn classify_distinguishes_the_three_link_forms() {
    let text = "[a](http://x) [b][id] [`Ty`]";
    let got = classified(text, ClassifyOptions::default());
    assert_eq!(
        got,
        vec![
            ("[a](http://x)", ConstructKind::InlineLink),
            ("[b][id]", ConstructKind::ReferenceLink),
            ("[`Ty`]", ConstructKind::IntraDocLink),
        ],
    );
}

#[test]
fn classify_separates_code_span_from_autolink_from_html() {
    let text = "`code` <https://x.example> <br>";
    let got = classified(text, ClassifyOptions::default());
    assert_eq!(
        got,
        vec![
            ("`code`", ConstructKind::CodeSpan),
            ("<https://x.example>", ConstructKind::Autolink),
            ("<br>", ConstructKind::HtmlTag),
        ],
    );
}

#[test]
fn classify_atx_heading_excludes_the_newline() {
    let text = "# Title\nbody";
    let got = classified(text, ClassifyOptions::default());
    assert_eq!(got, vec![("# Title", ConstructKind::Heading)]);
}

#[test]
fn classify_setext_underline_after_text_is_a_heading() {
    let text = "Title\n===\n";
    let got = classified(text, ClassifyOptions::default());
    assert_eq!(got, vec![("===", ConstructKind::Heading)]);
}

#[test]
fn classify_thematic_break_after_blank_line_is_not_a_heading() {
    // A `---` not preceded by a paragraph line is a thematic break, not
    // a Setext underline; it is left unclassified.
    let text = "\n---\n";
    assert!(classify_constructs(text, ClassifyOptions::default()).is_empty());
}

#[test]
fn classify_reference_definition_and_link_both_reported() {
    let text = "see [x][id]\n\n[id]: http://example.com";
    let got = classified(text, ClassifyOptions::default());
    assert_eq!(
        got,
        vec![
            ("[x][id]", ConstructKind::ReferenceLink),
            (
                "[id]: http://example.com",
                ConstructKind::ReferenceDefinition
            ),
        ],
    );
}

#[test]
fn emphasis_only_classified_when_requested() {
    let text = "**bold** and *italic*";
    assert!(classify_constructs(text, ClassifyOptions::default()).is_empty());
    let opts = ClassifyOptions {
        detect_emphasis: true,
        detect_lists: false,
    };
    assert_eq!(
        classified(text, opts),
        vec![
            ("**bold**", ConstructKind::Bold),
            ("*italic*", ConstructKind::Italic),
        ],
    );
}

#[test]
fn intraword_underscore_is_not_emphasis() {
    let opts = ClassifyOptions {
        detect_emphasis: true,
        detect_lists: false,
    };
    assert!(classify_constructs("foo_bar_baz", opts).is_empty());
}

#[test]
fn list_markers_only_classified_when_requested() {
    let text = "- one\n- two";
    assert!(classify_constructs(text, ClassifyOptions::default()).is_empty());
    let opts = ClassifyOptions {
        detect_emphasis: false,
        detect_lists: true,
    };
    let got = classified(text, opts);
    assert_eq!(
        got,
        vec![("-", ConstructKind::List), ("-", ConstructKind::List)],
    );
}
