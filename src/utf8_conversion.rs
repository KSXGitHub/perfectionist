//! Recognising a UTF-8 conversion of a value whose native form is not
//! UTF-8, and the lossless views that value offers in place of the text.
//!
//! The conversions are the ones the standard library offers for
//! reaching UTF-8 text from an OS string, a byte buffer or a C string.
//! What a rule does with one is its own business: this module answers
//! what the conversion is, which value it converted, and what that
//! value's native form offers instead.
//!
//! Two rules want that answer and differ in what consumes the text.
//! `utf8_conversion_for_literal_comparison` reads a conversion whose
//! result only meets string literals, and the planned
//! `needless_utf8_conversion` reads one whose result feeds a sink that
//! accepts the native form already.

use clippy_utils::paths::{PathNS, lookup_path};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::{Symbol, sym};

/// A value's native representation, where that representation is not
/// UTF-8.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NativeForm {
    /// `OsStr` or `OsString`.
    OsString,
    /// `Path` or `PathBuf`.
    Path,
    /// `[u8]`, `Vec<u8>` or `Box<[u8]>`.
    Bytes,
    /// `CStr`.
    CStr,
    /// `CString`.
    CString,
}

impl NativeForm {
    /// The calls that turn the value into a byte slice, appended to the
    /// value's own text.
    ///
    /// A `*_with_nul` view is deliberately absent for the C-string
    /// forms. It retains the terminator, which no string literal can
    /// carry, so pairing one with a plain literal yields a comparison
    /// that compiles and never matches.
    pub(crate) fn byte_view(self) -> &'static str {
        match self {
            NativeForm::OsString => ".as_encoded_bytes()",
            NativeForm::Path => ".as_os_str().as_encoded_bytes()",
            NativeForm::Bytes => "",
            NativeForm::CStr => ".to_bytes()",
            NativeForm::CString => ".as_bytes()",
        }
    }

    /// The calls that turn the value into an `&OsStr`, for the forms
    /// that have one.
    ///
    /// An `OsStr` carries `PartialEq<str>`, so an equality reaching one
    /// meets a string literal and keeps the literal's text, which is
    /// why this is asked separately from [`NativeForm::byte_view`]. A
    /// byte buffer and a C string answer `None`: neither reaches an
    /// `OsStr`, so an equality on one goes through the byte view like a
    /// pattern does.
    pub(crate) fn os_str_view(self) -> Option<&'static str> {
        match self {
            NativeForm::OsString => Some(""),
            NativeForm::Path => Some(".as_os_str()"),
            NativeForm::Bytes | NativeForm::CStr | NativeForm::CString => None,
        }
    }
}

/// A UTF-8 conversion, and the value it converted.
pub(crate) struct Conversion<'tcx> {
    /// The converted value: the receiver of a method, or the argument of
    /// an associated function.
    pub(crate) source: &'tcx Expr<'tcx>,
    /// `source`'s native form.
    pub(crate) native: NativeForm,
    /// Whether the conversion replaces ill-formed input with U+FFFD
    /// rather than reporting it.
    ///
    /// A replacement is what makes a literal holding U+FFFD unsafe to
    /// compare as bytes, since such a literal has pre-images the byte
    /// comparison does not share.
    pub(crate) lossy: bool,
}

/// The conversion `expr` performs, where it performs one.
///
/// Both method and associated-function spellings answer here, so a
/// caller need not know which of the two reached the text.
pub(crate) fn conversion<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<Conversion<'tcx>> {
    match expr.kind {
        ExprKind::MethodCall(segment, receiver, [], _) => {
            let lossy = method_conversion(segment.ident.name)?;
            Some(Conversion {
                source: receiver,
                native: native_form(cx, receiver)?,
                lossy,
            })
        }
        ExprKind::Call(callee, [argument]) => {
            let lossy = function_conversion(cx, callee)?;
            Some(Conversion {
                source: argument,
                native: native_form(cx, argument)?,
                lossy,
            })
        }
        _ => None,
    }
}

/// Whether `name` is a conversion taken as a method, and whether it is
/// lossy.
///
/// `into_string` belongs here beside the borrowing conversions even
/// though it consumes its receiver: the byte view the rewrite reaches
/// for borrows instead, which only widens what compiles.
fn method_conversion(name: Symbol) -> Option<bool> {
    let lossy = match name.as_str() {
        "to_str" | "into_string" => false,
        "to_string_lossy" => true,
        _ => return None,
    };
    Some(lossy)
}

/// Whether `callee` names a conversion taken as an associated function,
/// and whether it is lossy.
fn function_conversion(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<bool> {
    let ExprKind::Path(path) = callee.kind else {
        return None;
    };
    let definition = cx.qpath_res(&path, callee.hir_id).opt_def_id()?;
    let resolves_to =
        |segments: &[Symbol]| lookup_path(cx.tcx, PathNS::Value, segments).contains(&definition);
    let intern = Symbol::intern;
    if resolves_to(&[sym::core, sym::str, intern("from_utf8")])
        || resolves_to(&[
            sym::alloc,
            intern("string"),
            sym::String,
            intern("from_utf8"),
        ])
    {
        return Some(false);
    }
    if resolves_to(&[
        sym::alloc,
        intern("string"),
        sym::String,
        intern("from_utf8_lossy"),
    ]) {
        return Some(true);
    }
    None
}

/// `expr`'s native form, where its type has one that is not UTF-8.
///
/// References are peeled first, so a `&&OsStr` answers as the `OsStr`
/// it derefs to. The type query is what keeps the rule off a `str` that
/// merely shares a method name.
fn native_form(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<NativeForm> {
    let converted = cx.typeck_results().expr_ty(expr).peel_refs();
    if let ty::Slice(element) | ty::Array(element, _) = converted.kind() {
        return is_u8(*element).then_some(NativeForm::Bytes);
    }
    if converted.is_box() {
        return boxed_bytes(converted).then_some(NativeForm::Bytes);
    }
    let ty::Adt(adt, arguments) = converted.kind() else {
        return None;
    };
    let did = adt.did();
    // These types carry no diagnostic item under the names they are
    // written with, so the name plus the crate that defines it is what
    // identifies them. The crate test is what keeps a project's own
    // `Path` out.
    if !matches!(
        cx.tcx.crate_name(did.krate).as_str(),
        "std" | "core" | "alloc",
    ) {
        return None;
    }
    match cx.tcx.item_name(did).as_str() {
        "OsStr" | "OsString" => Some(NativeForm::OsString),
        "Path" | "PathBuf" => Some(NativeForm::Path),
        "CStr" => Some(NativeForm::CStr),
        "CString" => Some(NativeForm::CString),
        // A `Vec<u8>` is a byte buffer; a `Vec` over any other element
        // is not.
        "Vec" if is_u8(arguments.type_at(0)) => Some(NativeForm::Bytes),
        _ => None,
    }
}

/// Whether `boxed` is a `Box` over a slice of bytes.
fn boxed_bytes(boxed: ty::Ty<'_>) -> bool {
    boxed.boxed_ty().is_some_and(|inner| match inner.kind() {
        ty::Slice(element) => is_u8(*element),
        _ => false,
    })
}

/// Whether `ty` is `u8`.
fn is_u8(ty: ty::Ty<'_>) -> bool {
    matches!(ty.kind(), ty::Uint(ty::UintTy::U8))
}
