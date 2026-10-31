//! Reporting a conversion that only meets literals, and suggesting the
//! comparison to put in its place.
//!
//! The rewrite edits more than one span: an equality replaces both of
//! its operands, and a `match` replaces its scrutinee along with the
//! arms whose patterns change. A multipart suggestion is what expresses
//! either, so both shapes take that route.

use super::UTF8_CONVERSION_FOR_LITERAL_COMPARISON;
use super::consumer::Consumer;
use super::source::{Converted, Reached};
use crate::utf8_conversion::NativeForm;
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::peel_hir_expr_refs;
use clippy_utils::sugg::Sugg;
use rustc_errors::Applicability;
use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::{Span, Symbol, sym};

/// U+FFFD REPLACEMENT CHARACTER, what a lossy conversion puts in place
/// of each ill-formed sequence.
const REPLACEMENT: char = '�';

/// Which of the value's own forms the rewrite compares.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    /// The value as an `OsStr`, which meets a string literal through
    /// its `PartialEq<str>` and so keeps the literal's text.
    OsStr,
    /// The value's bytes, which meet a byte-string literal.
    Bytes,
}

/// Report `consumed` and suggest the comparison that reaches the same
/// answer without the conversion.
pub(super) fn report<'tcx>(
    cx: &LateContext<'tcx>,
    consumed: &'tcx Expr<'tcx>,
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
    let Some((edits, applicability)) = rewrite(cx, consumed.span, converted, consumer) else {
        return;
    };
    span_lint_and_then(
        cx,
        UTF8_CONVERSION_FOR_LITERAL_COMPARISON,
        consumed.span,
        "this converts to UTF-8 only to measure the result against string literals",
        |diag| {
            diag.multipart_suggestion(
                "compare the value without converting it",
                edits,
                applicability,
            );
        },
    );
}

/// Every edit the rewrite makes, with how far the result can be
/// trusted.
///
/// `None` says the rewrite cannot be spelled, because a span it would
/// edit belongs to a macro rather than to the author.
fn rewrite(
    cx: &LateContext<'_>,
    scrutinee: Span,
    converted: &Converted<'_>,
    consumer: &Consumer,
) -> Option<(Vec<(Span, String)>, Applicability)> {
    let reached = converted.reached;
    let view = view(converted.conversion.native, reached, consumer);
    let mut applicability = Applicability::MachineApplicable;
    let value = value_text(cx, converted, view, &mut applicability)?;
    let mut edits = Vec::new();
    match consumer {
        Consumer::Equality {
            operand,
            comparand,
            literal,
            drops_panic,
        } => {
            // The operand is replaced whole rather than at the
            // conversion, so an `unwrap` the comparison applied goes
            // with it. That drops a panic on ill-formed input, leaving
            // a rewrite that answers `false` where the original
            // panicked.
            edits.push((*operand, value));
            edits.push((*comparand, comparand_text(view, reached, *literal)));
            if *drops_panic {
                applicability = Applicability::MaybeIncorrect;
            }
        }
        Consumer::Patterns { arms, failure } => {
            edits.push((scrutinee, value));
            for arm in arms {
                edits.push((arm.pattern, pattern_text(view, reached, &arm.literals)));
            }
            // Reaching the conversion directly, the rewrite hands the
            // `match` the value's own form, which is neither an
            // `Option` nor a `Result`. An arm naming the conversion's
            // failure then has nothing left to name. An adapter's
            // `Option` survives the rewrite, so there the arm stands.
            if let (Reached::Directly, Some(pattern)) = (reached, *failure) {
                edits.push((pattern, "_".to_owned()));
            }
        }
    }
    // Every span the rewrite edits has to be the author's. An edit
    // inside an expansion would rewrite a macro's own text through one
    // of its call sites, and what the rule claims is about the text the
    // author wrote.
    edits
        .iter()
        .all(|(span, _)| !span.from_expansion())
        .then_some((edits, applicability))
}

/// Which form the rewrite compares the value in.
fn view(native: NativeForm, reached: Reached, consumer: &Consumer) -> View {
    // A pattern needs its literal in pattern position, where a string
    // literal matches a `str` and nothing else. An `Option`'s equality
    // is between two of the same type, so an `Option` of an `OsStr`
    // does not meet a `Some` of a string literal either. Both leave the
    // bytes as the one form a literal can be met in.
    if consumer.needs_pattern() || reached == Reached::ThroughOption {
        return View::Bytes;
    }
    match native.os_str_view() {
        Some(_) => View::OsStr,
        None => View::Bytes,
    }
}

/// The text replacing the operand or scrutinee that carried the
/// conversion.
fn value_text(
    cx: &LateContext<'_>,
    converted: &Converted<'_>,
    view: View,
    applicability: &mut Applicability,
) -> Option<String> {
    let native = converted.conversion.native;
    let source = converted.conversion.source;
    match converted.reached {
        Reached::Directly => {
            // The `&` the conversion needed is not the comparison's:
            // a literal meets the value through one layer of reference
            // at most, and the slicing a buffer may need has to reach
            // that buffer rather than a reference to it.
            let held = peel_hir_expr_refs(source).0;
            let value = Sugg::hir_with_applicability(cx, held, "..", applicability);
            let holds = cx.typeck_results().expr_ty(held);
            Some(match view {
                View::OsStr => with_suffix(value, native.os_str_view()?, holds),
                View::Bytes => bytes_text(cx, holds, value, native),
            })
        }
        Reached::ThroughOption => {
            let receiver = Sugg::hir_with_applicability(cx, source, "..", applicability);
            let receiver = receiver.maybe_paren();
            Some(match native.byte_view() {
                "" => receiver.to_string(),
                reaching => format!("{receiver}.map(|value| value{reaching})"),
            })
        }
    }
}

/// The value's bytes, with the slicing a buffer that is not already a
/// slice needs.
///
/// A byte-string literal is a reference to an array. That meets a slice
/// and a `Vec`, and meets an array only of the array's own length, so a
/// `Box<[u8]>` and a `[u8; N]` are sliced to reach a comparand they
/// meet whatever the literal's length.
fn bytes_text(
    cx: &LateContext<'_>,
    holds: ty::Ty<'_>,
    value: Sugg<'_>,
    native: NativeForm,
) -> String {
    if native != NativeForm::Bytes {
        return with_suffix(value, native.byte_view(), holds);
    }
    let already_a_slice = match holds.peel_refs().kind() {
        ty::Slice(_) => true,
        ty::Adt(buffer, _) => cx.tcx.is_diagnostic_item(sym::Vec, buffer.did()),
        _ => false,
    };
    if already_a_slice {
        with_suffix(value, "", holds)
    } else {
        format!("&{}[..]", value.maybe_paren())
    }
}

/// `value` with `suffix` appended, or, where there is no suffix,
/// dereferenced to the one layer of reference a comparison against a
/// literal reaches through.
///
/// A method call reaches through any number of references on its own,
/// so a suffix needs no dereference and only brackets, where the suffix
/// would otherwise bind to part of `value`. A bare operand does need
/// one: `&OsStr` meets `&str`, and `&[u8]` meets a byte-string literal,
/// through the blanket `PartialEq` between a pair of references, which
/// reaches through one layer and no further. A closure parameter is
/// where a second layer turns up, as in
/// `names.iter().any(|name| name.to_str() == Some("x"))`.
fn with_suffix(value: Sugg<'_>, suffix: &str, holds: ty::Ty<'_>) -> String {
    if !suffix.is_empty() {
        return format!("{}{suffix}", value.maybe_paren());
    }
    let mut layers = 0_usize;
    let mut holds = holds;
    while let ty::Ref(_, inner, _) = holds.kind() {
        layers += 1;
        holds = *inner;
    }
    if layers > 1 {
        format!("{}{}", "*".repeat(layers - 1), value.maybe_paren())
    } else {
        value.to_string()
    }
}

/// The comparand an equality measures the value against.
fn comparand_text(view: View, reached: Reached, literal: Symbol) -> String {
    let rendered = literal_text(view, literal);
    match reached {
        Reached::Directly => rendered,
        // The comparison already carries the `Some` the adapter's
        // `Option` answers with, so the rewrite keeps it. The adapter
        // now yields an `Option` of a slice, and a byte-string literal
        // is a reference to an array, so the literal is sliced to reach
        // the type the `Option` is compared at.
        Reached::ThroughOption => format!("Some({rendered}.as_slice())"),
    }
}

/// The pattern an arm names in place of the literals it named.
fn pattern_text(view: View, reached: Reached, literals: &[Symbol]) -> String {
    let rendered = literals
        .iter()
        .map(|literal| literal_text(view, *literal))
        .collect::<Vec<_>>()
        .join(" | ");
    match reached {
        Reached::Directly => rendered,
        // A byte-string literal is a pattern for a slice as much as
        // for an array, so the `Some` is all this needs.
        Reached::ThroughOption => format!("Some({rendered})"),
    }
}

/// `literal` respelled to meet the form being compared.
fn literal_text(view: View, literal: Symbol) -> String {
    let text = literal.as_str();
    match view {
        View::OsStr => format!("{text:?}"),
        View::Bytes => byte_literal(text),
    }
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
