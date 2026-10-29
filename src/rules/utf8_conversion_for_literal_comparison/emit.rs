//! Reporting a conversion that only meets literals, and naming the
//! comparison to put in its place.
//!
//! The replacement is offered as help rather than as an applied edit.
//! A pattern consumer's rewrite changes the scrutinee and every arm at
//! once, which one span replacement cannot express, and an equality's
//! rewrite is named the same way so both read alike.

use super::UTF8_CONVERSION_FOR_LITERAL_COMPARISON;
use super::consumer::Consumer;
use super::source::{Converted, Reached};
use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::source::snippet_opt;
use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// U+FFFD REPLACEMENT CHARACTER, what a lossy conversion puts in place
/// of each ill-formed sequence.
const REPLACEMENT: char = '�';

/// Report `expr` and name the comparison that reaches the same answer
/// without the conversion.
pub(super) fn report<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    converted: &Converted<'tcx>,
    consumer: &Consumer,
) {
    // A lossy conversion maps every ill-formed sequence to U+FFFD, so a
    // literal holding one is matched by inputs whose bytes differ from
    // it. The byte comparison has no such pre-images, so the rewrite
    // would answer differently and the site is left alone.
    if converted.conversion.lossy
        && consumer
            .literals()
            .iter()
            .map(Symbol::as_str)
            .any(|text| text.contains(REPLACEMENT))
    {
        return;
    }
    let Some(source) = snippet_opt(cx, converted.conversion.source.span) else {
        return;
    };
    span_lint_and_help(
        cx,
        UTF8_CONVERSION_FOR_LITERAL_COMPARISON,
        expr.span,
        "this converts to UTF-8 only to measure the result against string literals",
        None,
        replacement(&source, converted, consumer),
    );
}

/// The comparison to name in the help, given the value's own text.
fn replacement(source: &str, converted: &Converted<'_>, consumer: &Consumer) -> String {
    let native = converted.conversion.native;
    // A pattern needs its literal in pattern position, and
    // `OsStr::new("x")` is a function call, which a pattern cannot be.
    // So the byte view is the only route for a `match`, whatever the
    // source's native form offers an equality.
    let os_str_view = if consumer.needs_pattern() {
        None
    } else {
        native.os_str_view()
    };
    let literals = consumer.literals();
    match (os_str_view, converted.reached) {
        (Some(view), Reached::Directly) => format!(
            "compare `{source}{view}` against {}",
            wrapped(literals, "", os_str_call),
        ),
        // The comparison already carries the `Some` the adapter's
        // `Option` answers with, so the rewrite keeps it and drops only
        // the conversion inside the closure.
        (Some(_), Reached::ThroughOption) => format!(
            "compare `{source}{}` against {}",
            map_to_os_str(native),
            wrapped(literals, "Some", os_str_call),
        ),
        (None, Reached::Directly) => format!(
            "compare `{source}{}` against {}",
            native.byte_view(),
            wrapped(literals, "", |literal| byte_literal(literal.as_str())),
        ),
        (None, Reached::ThroughOption) => format!(
            "compare `{source}{}` against {}",
            map_to_bytes(native),
            wrapped(literals, "Some", |literal| byte_literal(literal.as_str())),
        ),
    }
}

/// The adapter that turns an `Option` of the native form into an
/// `Option` of an `&OsStr`.
///
/// An `OsStr` payload is already what the comparison wants, so the
/// adapter is dropped entirely there.
fn map_to_os_str(native: crate::utf8_conversion::NativeForm) -> String {
    match native.os_str_view() {
        Some("") | None => String::new(),
        Some(view) => format!(".map(|value| value{view})"),
    }
}

/// The adapter that turns an `Option` of the native form into an
/// `Option` of a byte slice.
fn map_to_bytes(native: crate::utf8_conversion::NativeForm) -> String {
    let view = native.byte_view();
    if view.is_empty() {
        String::new()
    } else {
        format!(".map(|value| value{view})")
    }
}

/// `OsStr::new` applied to `literal`, as the help spells it.
fn os_str_call(literal: Symbol) -> String {
    format!("OsStr::new({:?})", literal.as_str())
}

/// `literals` rendered by `render`, each inside `wrapper` where one is
/// named, and joined for prose.
///
/// The separator is a comma throughout rather than an "and" before the
/// last, so the help reads the same however many literals the
/// comparison named.
fn wrapped(literals: &[Symbol], wrapper: &str, render: impl Fn(Symbol) -> String) -> String {
    literals
        .iter()
        .map(|literal| {
            let rendered = render(*literal);
            if wrapper.is_empty() {
                format!("`{rendered}`")
            } else {
                format!("`{wrapper}({rendered})`")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `text` respelled as a byte-string literal.
///
/// A byte-string literal admits only ASCII, so anything above it is
/// written as the `\xHH` escapes of its UTF-8 bytes. The quote, the
/// backslash and the control characters are escaped for the same reason
/// a string literal escapes them.
fn byte_literal(text: &str) -> String {
    let mut rendered = String::from(r#"b""#);
    for byte in text.bytes() {
        match byte {
            b'"' => rendered.push_str(r#"\""#),
            b'\\' => rendered.push_str(r"\\"),
            b'\n' => rendered.push_str(r"\n"),
            b'\r' => rendered.push_str(r"\r"),
            b'\t' => rendered.push_str(r"\t"),
            0x20..=0x7E => rendered.push(byte as char),
            _ => rendered.push_str(&format!(r"\x{byte:02x}")),
        }
    }
    rendered.push('"');
    rendered
}

#[cfg(test)]
mod tests {
    use super::byte_literal;

    /// What a literal's byte-string spelling has to survive: ASCII
    /// passes through, a non-ASCII character becomes the `\xHH` escapes
    /// of its UTF-8 bytes, and the characters a literal cannot hold raw
    /// are escaped.
    #[test]
    fn byte_literal_spells_every_literal() {
        assert_eq!(byte_literal("true"), r#"b"true""#);
        assert_eq!(byte_literal(""), r#"b"""#);
        assert_eq!(byte_literal("main.rs"), r#"b"main.rs""#);
        // `é` is two UTF-8 bytes, and `b"é"` does not compile.
        assert_eq!(byte_literal("é"), r#"b"\xc3\xa9""#);
        assert_eq!(byte_literal(r#"a"b"#), r#"b"a\"b""#);
        assert_eq!(byte_literal(r"a\b"), r#"b"a\\b""#);
        assert_eq!(byte_literal("a\nb"), r#"b"a\nb""#);
    }
}
