//! Helpers for reading `#[serde(...)]` and `#[doc = "..."]`
//! attributes off arbitrary syn items. Used by every layer of the
//! extractor: rule-level doc comment, Config / Type struct rename
//! rules, per-field renames, per-variant renames.

use syn::{Attribute, Expr, ExprLit, Lit, Meta, Token};

/// Read a `#[serde(key = "literal")]` string value, scanning every
/// `serde(...)` attribute on the item. Returns the first match in
/// source order; an attribute whose body fails to parse is silently
/// skipped, since this generator runs against rule sources that the
/// compiler has already accepted — a parse failure here means the
/// attribute uses a form (e.g. `serde(deny_unknown_fields)`) that
/// the generator simply doesn't surface.
pub(crate) fn serde_str_attr(attrs: &[Attribute], key: &str) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let parsed =
            attr.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated);
        let Ok(items) = parsed else { continue };
        for item in items {
            if let Meta::NameValue(name_value) = item
                && name_value.path.is_ident(key)
                && let Expr::Lit(ExprLit {
                    lit: Lit::Str(literal),
                    ..
                }) = name_value.value
            {
                return Some(literal.value());
            }
        }
    }
    None
}

/// Whether the item carries a serde `default` directive — the bare
/// flag `#[serde(default)]` or `#[serde(default = "path")]`. On a
/// container it means any missing field is filled from a default; on a
/// field it means that one field is. Used to tell a required config
/// field (no default, not `Option`) from an optional one.
pub(crate) fn serde_has_default(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let Ok(items) =
            attr.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            continue;
        };
        for item in items {
            let path = match &item {
                Meta::Path(path) => path,
                Meta::NameValue(name_value) => &name_value.path,
                Meta::List(list) => &list.path,
            };
            if path.is_ident("default") {
                return true;
            }
        }
    }
    false
}

/// Apply one of serde's `rename_all` styles to a Rust identifier.
/// Covers the styles serde itself documents: `snake_case`,
/// `kebab-case`, their SCREAMING variants, `lowercase`, `UPPERCASE`,
/// `camelCase`, and `PascalCase`. None of the current rules need
/// anything outside `snake_case`; the others are here so that
/// adopting a new style elsewhere doesn't silently mangle the
/// rendered TOML keys.
///
/// An unrecognised style prints a warning to stderr and falls
/// through to the identifier as written. Warning rather than
/// panicking, on the principle that a wrong doc page is less bad
/// than a broken doc-generation build — but the warning makes the
/// gap loud enough that the rule author notices.
pub(crate) fn apply_rename_all(style: &str, name: &str) -> String {
    match style {
        "snake_case" => pascal_to_snake(name),
        "SCREAMING_SNAKE_CASE" => pascal_to_snake(name).to_ascii_uppercase(),
        "kebab-case" => pascal_to_snake(name).replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => pascal_to_snake(name).replace('_', "-").to_ascii_uppercase(),
        "lowercase" => name.to_ascii_lowercase(),
        "UPPERCASE" => name.to_ascii_uppercase(),
        // Rust enum variants are already `PascalCase` by convention.
        "PascalCase" => name.to_owned(),
        // `camelCase` is `PascalCase` with the first character
        // lower-cased; everything else stays as written.
        "camelCase" => {
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => first.to_lowercase().chain(chars).collect(),
                None => String::new(),
            }
        }
        unknown => {
            eprintln!(
                "warning: unrecognised serde `rename_all` style {unknown:?}; \
                 rendering `{name}` unchanged. Add an arm to `apply_rename_all` \
                 if this style needs to be supported.",
            );
            name.to_owned()
        }
    }
}

/// Convert `PascalCase` (or `camelCase`) to `snake_case` by inserting
/// `_` before each uppercase letter that follows a lowercase one or
/// precedes a lowercase one. Adequate for the rule-author-controlled
/// enum names this generator sees; not a general-purpose case
/// converter.
pub(crate) fn pascal_to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    let chars: Vec<char> = name.chars().collect();
    for (index, &char) in chars.iter().enumerate() {
        if char.is_ascii_uppercase() {
            let prev_lower = index > 0 && chars[index - 1].is_ascii_lowercase();
            let next_lower = chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if index > 0 && (prev_lower || next_lower) {
                out.push('_');
            }
            out.push(char.to_ascii_lowercase());
        } else {
            out.push(char);
        }
    }
    out
}

pub(crate) fn doc_attrs_to_markdown(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &meta.value
        else {
            continue;
        };
        // `/// foo` lexes as `#[doc = " foo"]`; drop the single
        // convention-space so the markdown round-trips cleanly.
        let raw = s.value();
        let trimmed = raw.strip_prefix(' ').unwrap_or(&raw).to_owned();
        lines.push(trimmed);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;
