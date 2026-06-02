//! Identifier case-style classification, shared by the case-filtering
//! knobs (`check_pascal_case` / `check_upper_case` / `check_snake_case`)
//! and the minimum-word-count knob (`min_words`).

/// The naming-case style of an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Case {
    /// `snake_case` — lowercase letters and digits, words joined by
    /// single underscores (`foo`, `foo_bar`, `foo2`).
    Snake,
    /// `UPPER_CASE` / `SCREAMING_SNAKE_CASE` — uppercase letters and
    /// digits, words joined by single underscores (`FOO`, `FOO_BAR`).
    Upper,
    /// `PascalCase` — capitalised words, no underscores (`Foo`,
    /// `FooBar`).
    Pascal,
    /// A mixed or otherwise non-conformist spelling (`fooBar`,
    /// `foo_BAR`, `Foo_bar`, `__foo`, `foo__bar`, ...).
    NonConformist,
}

/// Classify an identifier's case style. A leading, trailing, or doubled
/// underscore, a mix of upper and lower outside of `PascalCase`, or any
/// non-`[A-Za-z0-9_]` byte all make the name [`Case::NonConformist`].
pub(super) fn classify(name: &str) -> Case {
    if name.is_empty() {
        return Case::NonConformist;
    }
    if name.starts_with('_') || name.ends_with('_') || name.contains("__") {
        return Case::NonConformist;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Case::NonConformist;
    }
    let has_lower = name.chars().any(|ch| ch.is_ascii_lowercase());
    let has_upper = name.chars().any(|ch| ch.is_ascii_uppercase());
    if !has_upper {
        // Only lowercase letters, digits, and single underscores.
        return Case::Snake;
    }
    if !has_lower {
        // Only uppercase letters, digits, and single underscores.
        return Case::Upper;
    }
    // Both cases present: `PascalCase` is the one conformist shape —
    // no underscores and an uppercase first letter.
    if !name.contains('_') && name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
        return Case::Pascal;
    }
    Case::NonConformist
}

/// Number of words in `name`, given its already-computed [`Case`].
///
/// `snake_case` / `UPPER_CASE` split on underscores; `PascalCase` splits
/// on camel humps (with the `HTTPServer` → `HTTP` + `Server` acronym
/// rule). A non-conformist name has no meaningful word count — callers
/// treat it as "checked regardless" and never consult this — so it
/// reports `1`.
pub(super) fn word_count(name: &str, case: Case) -> usize {
    match case {
        Case::Snake | Case::Upper => name
            .split('_')
            .filter(|segment| !segment.is_empty())
            .count()
            .max(1),
        Case::Pascal => pascal_word_count(name),
        Case::NonConformist => 1,
    }
}

/// Count `PascalCase` words by camel humps. A word begins at the first
/// character, at an uppercase letter that follows a non-uppercase one
/// (`fooBar` → `foo` + `Bar`), and at an uppercase letter that both
/// follows an uppercase one and is followed by a lowercase one
/// (`HTTPServer` → `HTTP` + `Server`).
fn pascal_word_count(name: &str) -> usize {
    let chars: Vec<char> = name.chars().collect();
    let mut count = 0;
    for (index, &ch) in chars.iter().enumerate() {
        let starts_word = if index == 0 {
            true
        } else if ch.is_ascii_uppercase() {
            let previous = chars[index - 1];
            let next_is_lower = chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
            !previous.is_ascii_uppercase() || next_is_lower
        } else {
            false
        };
        if starts_word {
            count += 1;
        }
    }
    count.max(1)
}

#[cfg(test)]
mod tests;
