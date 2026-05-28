use super::*;

#[test]
fn take_url_basic() {
    let url_match = take_url("https://example.com", DEFAULT_FORWARD_SCHEMES).unwrap();
    assert_eq!(url_match.url, "https://example.com");
    assert_eq!(url_match.consumed, url_match.url.len());
}

#[test]
fn take_url_stops_at_whitespace() {
    let url_match = take_url("https://example.com next word", DEFAULT_FORWARD_SCHEMES).unwrap();
    assert_eq!(url_match.url, "https://example.com");
}

#[test]
fn take_url_stops_at_closing_bracket() {
    let url_match = take_url("https://example.com>tail", DEFAULT_FORWARD_SCHEMES).unwrap();
    assert_eq!(url_match.url, "https://example.com");
}

#[test]
fn take_url_requires_scheme_match() {
    assert!(take_url("ftp://example.com", DEFAULT_FORWARD_SCHEMES).is_none());
}

#[test]
fn take_url_requires_double_slash() {
    assert!(take_url("https:example.com", DEFAULT_FORWARD_SCHEMES).is_none());
}

#[test]
fn take_url_rejects_empty_body() {
    assert!(take_url("https:// ", DEFAULT_FORWARD_SCHEMES).is_none());
}

#[test]
fn back_scan_detects_url_fragment() {
    let text = "see https://example.com/issues/#123";
    let pos = text.find('#').unwrap();
    assert!(back_scan_url_fragment(text, pos));
}

#[test]
fn back_scan_rejects_plain_hash() {
    let text = "fixes #123";
    let pos = text.find('#').unwrap();
    assert!(!back_scan_url_fragment(text, pos));
}

#[test]
fn back_scan_rejects_wrong_scheme() {
    let text = "see weird://example.com/issues/#123";
    let pos = text.find('#').unwrap();
    assert!(!back_scan_url_fragment(text, pos));
}

#[test]
fn classify_trailing_alnum_is_safe() {
    assert_eq!(
        classify_trailing("https://example.com/abc", &['_']),
        TrailingClass::Safe,
    );
}

#[test]
fn classify_trailing_dot_is_ambiguous() {
    assert_eq!(
        classify_trailing("https://example.com.", &['_']),
        TrailingClass::Ambiguous,
    );
}

#[test]
fn classify_trailing_safe_char_overrides() {
    assert_eq!(
        classify_trailing("https://example.com/path_", &['_']),
        TrailingClass::Safe,
    );
}
