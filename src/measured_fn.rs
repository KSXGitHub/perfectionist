//! Which functions the per-function size and complexity rules
//! measure, decided once for all of them.
//!
//! Each of those rules is handed every body a crate has — closures,
//! macro-generated functions, test helpers — and measures the same
//! subset: a function or method the author wrote, and only test code
//! when the rule's `exempt_tests` is off. A closure is part of
//! the function that contains it; a function produced by a macro is
//! nothing the author can split; and test code is exempt on request
//! through [`crate::test_code::fn_in_test_code`].

use crate::test_code::fn_in_test_code;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

/// A function the rule measures: what to call it in the diagnostic and
/// where to anchor the diagnostic — its signature.
pub(crate) struct MeasuredFn {
    pub(crate) name: Symbol,
    pub(crate) kind_label: &'static str,
    pub(crate) span: Span,
}

/// The function `check_fn` was handed, or `None` when it is a closure,
/// produced by a macro, or test code that `exempt_tests` leaves
/// alone.
pub(crate) fn measured_fn(
    cx: &LateContext<'_>,
    kind: FnKind<'_>,
    def_id: LocalDefId,
    exempt_tests: bool,
) -> Option<MeasuredFn> {
    let (ident, kind_label) = match kind {
        FnKind::ItemFn(ident, ..) => (ident, "function"),
        FnKind::Method(ident, ..) => (ident, "method"),
        FnKind::Closure => return None,
    };
    let span = cx.tcx.def_span(def_id);
    if span.from_expansion() {
        return None;
    }
    if exempt_tests && fn_in_test_code(cx, def_id) {
        return None;
    }
    Some(MeasuredFn {
        name: ident.name,
        kind_label,
        span,
    })
}
