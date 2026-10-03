//! Recognising a method whose whole body copies one field of `self`.
//!
//! What a method's name then makes of that shape is the caller's
//! business: `cloning_getter` reads it as a getter that should have
//! handed back a borrow. Keeping the recognition in one place means
//! every rule that consults it agrees on what counts, so a method
//! cannot fall between two of them or be reported by both.
//!
//! The recognised shape is narrow on purpose: an inherent method
//! taking `&self` and nothing else, whose body — once statement-free
//! blocks are unwrapped — is exactly `self.<field>.<copying method>()`
//! producing the field's own type. A call that renders the field
//! (`to_string` on a number), one that returns a `Copy` value, and one
//! that clones a refcounted handle are none of them a copy a borrow
//! could have replaced, so none is recognised here.

use crate::common::hir_in_external_macro;
use clippy_utils::ty::is_copy;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Expr, ExprKind, ImplicitSelfKind, QPath};
use rustc_lint::LateContext;
use rustc_middle::ty::print::ForceTrimmedGuard;
use rustc_middle::ty::{self, AssocContainer, Ty};
use rustc_span::{Span, Symbol, kw};

/// The methods that turn a borrowed field into its owned form.
pub(crate) const COPYING_METHODS: &[&str] = &[
    "clone",
    "to_owned",
    "to_string",
    "to_vec",
    "to_path_buf",
    "to_os_string",
];

/// A method this family of rules may measure: an inherent method taking
/// `&self` and nothing else, written by hand rather than by a macro.
pub(crate) struct Eligible {
    /// The method's own name, for the diagnostic.
    pub(crate) method: Symbol,
    /// The method's signature, without its body.
    pub(crate) def_span: Span,
}

/// Whether this method is one the family may measure at all, separately
/// from what its body or its return type says. A trait impl is excluded
/// because the trait fixes the signature, and a macro-written method
/// because no reader can change it.
pub(crate) fn eligible_method<'tcx>(
    cx: &LateContext<'tcx>,
    kind: FnKind<'tcx>,
    decl: &'tcx hir::FnDecl<'tcx>,
    body: &'tcx hir::Body<'tcx>,
    def_id: LocalDefId,
) -> Option<Eligible> {
    let FnKind::Method(ident, _) = kind else {
        return None;
    };
    if !matches!(decl.implicit_self(), ImplicitSelfKind::RefImm) || decl.inputs.len() != 1 {
        return None;
    }
    let def_span = cx.tcx.def_span(def_id);
    let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
    // A proc-macro derive can span a generated method over the field it
    // reads, so the span alone cannot tell the two apart.
    if def_span.from_expansion()
        || hir_in_external_macro(cx, hir_id, def_span)
        || clippy_utils::is_from_proc_macro(cx, &(&kind, body, hir_id, def_span))
    {
        return None;
    }
    // A trait fixes the signature of its methods.
    if let Some(assoc) = cx.tcx.opt_associated_item(def_id.to_def_id())
        && !matches!(assoc.container, AssocContainer::InherentImpl)
    {
        return None;
    }
    Some(Eligible {
        method: ident.name,
        def_span,
    })
}

/// A field a method copies out, and the span to report it at.
pub(crate) struct FieldCopy<'tcx> {
    /// The method's own name, for the diagnostic.
    pub(crate) method: Symbol,
    /// The field the body copies.
    pub(crate) field: Symbol,
    /// The field's type, for suggesting its borrowed form.
    pub(crate) field_ty: Ty<'tcx>,
    /// The type `self` belongs to, for asking what fields it has.
    pub(crate) self_ty: Ty<'tcx>,
    /// The method's signature, without its body.
    pub(crate) def_span: Span,
}

/// The field this method copies out, or `None` when the method is not
/// the recognised shape or sits in code no reader can change.
pub(crate) fn field_copy<'tcx>(
    cx: &LateContext<'tcx>,
    kind: FnKind<'tcx>,
    decl: &'tcx hir::FnDecl<'tcx>,
    body: &'tcx hir::Body<'tcx>,
    def_id: LocalDefId,
    copying_methods: &[Symbol],
) -> Option<FieldCopy<'tcx>> {
    let eligible = eligible_method(cx, kind, decl, body, def_id)?;
    field_copy_of(cx, &eligible, body, def_id, copying_methods)
}

/// The same recognition, for a caller that has already established
/// eligibility and would otherwise pay for it twice. Deciding that
/// costs a source re-lex to rule out a proc macro, so a rule reading
/// both a method's body and its signature asks once and passes the
/// answer here.
pub(crate) fn field_copy_of<'tcx>(
    cx: &LateContext<'tcx>,
    eligible: &Eligible,
    body: &'tcx hir::Body<'tcx>,
    def_id: LocalDefId,
    copying_methods: &[Symbol],
) -> Option<FieldCopy<'tcx>> {
    let &Eligible { method, def_span } = eligible;
    let typeck = cx.tcx.typeck(def_id);
    let expr = unwrap_block(body.value);
    let ExprKind::MethodCall(segment, receiver, [], _) = expr.kind else {
        return None;
    };
    if !copying_methods.contains(&segment.ident.name) {
        return None;
    }
    let ExprKind::Field(base, field) = receiver.kind else {
        return None;
    };
    let ExprKind::Path(QPath::Resolved(None, path)) = base.kind else {
        return None;
    };
    let [base_segment] = path.segments else {
        return None;
    };
    if base_segment.ident.name != kw::SelfLower {
        return None;
    }
    let self_ty = typeck.expr_ty(base);
    let field_ty = typeck.expr_ty(receiver);
    // The call has to reproduce the field's own type for a borrow of the
    // field to serve in its place. `self.count.to_string()` renders a
    // `u32`, and no borrow of `self.count` is a `String`.
    if typeck.expr_ty(expr) != field_ty {
        return None;
    }
    // A `Copy` field returned by value is the borrowed form's equal. A
    // shared reference is itself `Copy`, so this also leaves alone a
    // field that is already a borrow, where the call copies nothing.
    if is_copy(cx, field_ty) {
        return None;
    }
    // Cloning an `Rc` or an `Arc` bumps a refcount rather than copying
    // what the handle points at, so the borrowed form saves nothing,
    // and `&Rc<T>` does not serve in its place: a caller that keeps the
    // handle has to own one. Measuring it would also make the rule
    // depend on spelling, since the same clone written
    // `Rc::clone(&self.field)` -- the form `clippy::clone_on_ref_ptr`
    // asks for -- is a call rather than a method call and never reaches
    // this far.
    if is_refcounted_handle(cx, field_ty) {
        return None;
    }
    Some(FieldCopy {
        method,
        field: field.name,
        field_ty,
        self_ty,
        def_span,
    })
}

/// Whether `name` is the name of a field of `self_ty`. A tuple struct's
/// fields are named `0`, `1`, ..., which no method can be called, so such
/// a type simply never matches.
pub(crate) fn has_field<'tcx>(self_ty: Ty<'tcx>, name: Symbol) -> bool {
    let ty::Adt(adt, _) = self_ty.peel_refs().kind() else {
        return false;
    };
    adt.all_fields().any(|field| field.name == name)
}

/// Whether `ty` is an `Rc` or an `Arc`, whose `clone` copies a
/// refcount rather than the value behind it.
fn is_refcounted_handle<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    let ty::Adt(adt, _) = ty.kind() else {
        return false;
    };
    let did = adt.did();
    ["Rc", "Arc"]
        .iter()
        .any(|name| cx.tcx.is_diagnostic_item(Symbol::intern(name), did))
}

/// The borrowed form a caller could take in place of the field's owned
/// type: `&str` for a `String`, `&Path` for a `PathBuf`, `&OsStr` for an
/// `OsString`, `&CStr` for a `CString`, `&[T]` for a `Vec<T>`, the inner
/// type's own borrowed form under an `Option` or a `Box`, and `&T` for
/// anything else.
pub(crate) fn borrowed_form<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> String {
    // Print paths trimmed to their final segment, so the help reads
    // `&[String]` rather than `&[std::string::String]`. The guard is what
    // `with_forced_trimmed_paths!` expands to, held here for the whole
    // function rather than wrapped around each `format!`.
    let _trimmed = ForceTrimmedGuard::new();
    let ty::Adt(adt, args) = ty.kind() else {
        return format!("&{ty}");
    };
    let did = adt.did();
    // `String` is a lang item (`#[lang = "String"]`), not a diagnostic
    // item, so it needs its own lookup; the rest carry a
    // `rustc_diagnostic_item`.
    if Some(did) == cx.tcx.lang_items().string() {
        return "&str".to_owned();
    }
    let is = |name: &str| cx.tcx.is_diagnostic_item(Symbol::intern(name), did);
    if is("PathBuf") {
        return "&Path".to_owned();
    }
    if is("OsString") {
        return "&OsStr".to_owned();
    }
    // `CString` carries `cstring_type`; `needless_borrowed_parameters`
    // maps the same pair in the other direction.
    if is("cstring_type") {
        return "&CStr".to_owned();
    }
    let Some(inner) = args.types().next() else {
        return format!("&{ty}");
    };
    if is("Vec") {
        return format!("&[{inner}]");
    }
    if is("Option") {
        return format!("Option<{}>", borrowed_form(cx, inner));
    }
    // A `Box<T>` is owned storage for one `T`, so what a caller borrows
    // is the `T`: `&str` for a `Box<str>`, `&[T]` for a `Box<[T]>`.
    // `&Box<T>` would name the field's representation, which is the
    // habit this rule exists to break, and trips `clippy::borrowed_box`
    // besides.
    if Some(did) == cx.tcx.lang_items().owned_box() {
        return borrowed_form(cx, inner);
    }
    format!("&{ty}")
}

/// The expression a body of nested `{ }` blocks with no statements
/// comes down to.
fn unwrap_block<'a>(mut expr: &'a Expr<'a>) -> &'a Expr<'a> {
    while let ExprKind::Block(block, None) = expr.kind
        && block.stmts.is_empty()
        && let Some(inner) = block.expr
    {
        expr = inner;
    }
    expr
}
