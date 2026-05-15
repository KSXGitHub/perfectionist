//! `X.Y.Z` or `X.Y.Z-<suffix>`, where `<suffix>` is non-empty and
//! whitespace-free.

pub(crate) fn is_version_literal(input: &str) -> bool {
    parse_version_literal(input).is_some()
}

fn parse_version_literal(input: &str) -> Option<()> {
    let (_, rest) = take_digits(input)?;
    let rest = rest.strip_prefix('.')?;
    let (_, rest) = take_digits(rest)?;
    let rest = rest.strip_prefix('.')?;
    let (_, rest) = take_digits(rest)?;
    if rest.is_empty() {
        return Some(());
    }
    let suffix = rest.strip_prefix('-')?;
    (!suffix.is_empty() && !suffix.chars().any(char::is_whitespace)).then_some(())
}

/// Take a non-empty run of ASCII digits from the front of `input`,
/// returning `(digits, rest)`.
fn take_digits(input: &str) -> Option<(&str, &str)> {
    let end = input.bytes().take_while(|b| b.is_ascii_digit()).count();
    (end > 0).then(|| input.split_at(end))
}
