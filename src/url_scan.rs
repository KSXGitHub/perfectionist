//! URL discovery shared between `bare_url` (forward scan) and
//! `bare_issue_reference` (backward scan for fragment detection).
//!
//! The two consumers walk the same grammar in opposite directions:
//!
//! - `bare_url` commits forward from a candidate `http://` / `https://`
//!   prefix to determine where the URL ends.
//! - `bare_issue_reference` walks backward from a candidate `#N` token
//!   to determine whether it sits inside a URL fragment such as
//!   `https://example.com/issues/#123`.
//!
//! The grammar is deliberately small — a scheme run, a non-whitespace
//! body, optional trailing punctuation classification — per the
//! parser-combinator convention in
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`. No regex.

/// Result of [`take_url`]: a `&str` slice covering the URL body and
/// the remainder of the input after it.
pub(crate) struct UrlMatch<'a> {
    /// The matched URL text (scheme + `://` + body).
    pub(crate) url: &'a str,
    /// Number of bytes from the start of the caller's input that the
    /// URL occupies.
    pub(crate) consumed: usize,
}

/// URL schemes the forward scanner ([`take_url`]) recognises — the
/// schemes `bare_url` flags as bare URLs in comments. A narrow
/// subset of the wider [`BACKWARD_URL_SCHEMES`] used by the
/// `#N`-fragment back-scan: the wrapping concern that motivates
/// `bare_url` applies to `http` and `https` URLs, the only schemes
/// commonly written as prose links in doc comments.
pub(crate) const DEFAULT_FORWARD_SCHEMES: &[&str] = &["http", "https"];

/// All schemes the backward scan recognises when classifying a `#N`
/// candidate as a URL fragment. Wider than the forward scan's set —
/// `bare_issue_reference` does not want to flag fragments of `ssh://`
/// or `git://` URLs just because `bare_url` doesn't surface them.
pub(crate) const BACKWARD_URL_SCHEMES: &[&str] = &[
    "http", "https", "ftp", "ftps", "git", "ssh", "file", "mailto",
];

/// Take a URL starting at the beginning of `input`. Returns `None` if
/// `input` does not start with one of the configured schemes followed
/// by `://`.
///
/// The URL body extends greedily over non-whitespace bytes, but stops
/// before delimiters that would break a `<URL>` wrap or an enclosing
/// markdown link: `<`, `>`, `[`, `]`, `)`. Trailing punctuation is
/// *kept inside* the returned slice — the caller (`bare_url`)
/// classifies the last byte to decide `MachineApplicable` vs
/// `MaybeIncorrect`.
pub(crate) fn take_url<'a>(input: &'a str, schemes: &[&str]) -> Option<UrlMatch<'a>> {
    let scheme_len = take_scheme(input, schemes)?;
    let bytes = input.as_bytes();
    // Past the scheme there must be `://`.
    let after_scheme = scheme_len;
    if after_scheme + 3 > bytes.len()
        || bytes[after_scheme] != b':'
        || bytes[after_scheme + 1] != b'/'
        || bytes[after_scheme + 2] != b'/'
    {
        return None;
    }
    let body_start = after_scheme + 3;
    let mut index = body_start;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r' {
            break;
        }
        if byte == b'>' || byte == b']' || byte == b')' || byte == b'<' || byte == b'[' {
            break;
        }
        index += 1;
    }
    if index == body_start {
        return None;
    }
    Some(UrlMatch {
        url: &input[..index],
        consumed: index,
    })
}

/// Take the leading scheme — one or more ASCII letters — from
/// `input`, returning the byte length on success. Only schemes that
/// match (case-insensitively) one of the entries in `schemes` are
/// accepted.
fn take_scheme(input: &str, schemes: &[&str]) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
        index += 1;
    }
    if index == 0 {
        return None;
    }
    let candidate = &input[..index];
    for scheme in schemes {
        if candidate.eq_ignore_ascii_case(scheme) {
            return Some(index);
        }
    }
    None
}

/// Whether the byte position `pos` of `text` falls inside a URL
/// whose scheme appears in [`BACKWARD_URL_SCHEMES`]. Walks backward
/// from `pos` looking for `<scheme>://` with no intervening
/// whitespace.
///
/// Used by `bare_issue_reference` to suppress `#N` matches that are
/// the fragment of a URL such as
/// `https://example.com/issues/#123` — see the planning file's
/// "URL-fragment detection" note.
pub(crate) fn back_scan_url_fragment(text: &str, pos: usize) -> bool {
    let bytes = text.as_bytes();
    if pos > bytes.len() {
        return false;
    }
    // Walk backwards over non-whitespace bytes looking for `://`.
    let mut index = pos;
    while index > 0 {
        let prev = bytes[index - 1];
        if prev == b' ' || prev == b'\t' || prev == b'\n' || prev == b'\r' {
            return false;
        }
        // Look for `://` ending at index `index - 1`.
        if prev == b'/' && index >= 3 && bytes[index - 2] == b'/' && bytes[index - 3] == b':' {
            // Scheme letters end at index `index - 4`.
            let mut scheme_start = index - 3;
            while scheme_start > 0 && bytes[scheme_start - 1].is_ascii_alphabetic() {
                scheme_start -= 1;
            }
            if scheme_start == index - 3 {
                return false;
            }
            let scheme = &text[scheme_start..index - 3];
            for sc in BACKWARD_URL_SCHEMES {
                if scheme.eq_ignore_ascii_case(sc) {
                    return true;
                }
            }
            return false;
        }
        index -= 1;
    }
    false
}

/// Classification of a URL's last byte, used by `bare_url` to decide
/// whether the autofix substitution is machine-applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrailingClass {
    /// Trailing byte is unambiguously part of the URL (slash, alnum,
    /// or one of the configured `safe_trailing_chars`). The
    /// `<URL>` wrap is machine-applicable.
    Safe,
    /// Trailing byte could be either URL or sentence punctuation
    /// (`.`, `?`, `!`, `,`, `;`, `:`, `'`, `"`). The caller — today,
    /// only `bare_url` — degrades the suggestion to `MaybeIncorrect`
    /// so the author decides whether the trailing byte belongs to
    /// the URL or to the surrounding sentence. A future iteration
    /// may emit two suggestions (one keeping the trailing byte
    /// inside `<...>`, one moving it outside) once the
    /// trailing-punctuation split is implemented.
    Ambiguous,
}

/// Classify the last character of `url` against the supplied
/// `safe_trailing_chars` set. URL-safe characters defined by the
/// caller, plus every ASCII alphanumeric and `/`, are always
/// classified as [`TrailingClass::Safe`]; any other trailing byte is
/// [`TrailingClass::Ambiguous`].
pub(crate) fn classify_trailing(url: &str, safe_trailing_chars: &[char]) -> TrailingClass {
    let Some(last) = url.chars().last() else {
        return TrailingClass::Safe;
    };
    if last.is_ascii_alphanumeric() || last == '/' {
        return TrailingClass::Safe;
    }
    if safe_trailing_chars.contains(&last) {
        return TrailingClass::Safe;
    }
    TrailingClass::Ambiguous
}

#[cfg(test)]
mod tests {
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
}
