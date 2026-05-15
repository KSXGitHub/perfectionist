//! `X.Y.Z` or `X.Y.Z-<suffix>`, where `<suffix>` is non-empty and
//! whitespace-free.

pub(crate) fn is_version_literal(input: &str) -> bool {
    let Some((_, rest)) = take_digits(input) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix('.') else {
        return false;
    };
    let Some((_, rest)) = take_digits(rest) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix('.') else {
        return false;
    };
    let Some((_, rest)) = take_digits(rest) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let Some(suffix) = rest.strip_prefix('-') else {
        return false;
    };
    !suffix.is_empty() && !suffix.chars().any(char::is_whitespace)
}

/// Take a non-empty run of ASCII digits from the front of `input`,
/// returning `(digits, rest)`.
fn take_digits(input: &str) -> Option<(&str, &str)> {
    let end = input.bytes().take_while(|b| b.is_ascii_digit()).count();
    (end > 0).then(|| input.split_at(end))
}
