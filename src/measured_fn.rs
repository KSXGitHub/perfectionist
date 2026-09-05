//! Which functions the per-function size and complexity rules
//! measure, decided once for all of them.
//!
//! Each of those rules is handed every body a crate has — closures,
//! macro-generated functions, test helpers — and measures the same
//! subset: a function or method the author wrote, and only test code
//! when the rule's `test_code_exception` is off. A closure is part of
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
    pub(crate) span: Span,
}

/// The function `check_fn` was handed, or `None` when it is a closure,
/// produced by a macro, or test code that `test_code_exception` leaves
/// alone.
pub(crate) fn measured_fn(
    cx: &LateContext<'_>,
    kind: FnKind<'_>,
    def_id: LocalDefId,
    test_code_exception: bool,
) -> Option<MeasuredFn> {
    let (FnKind::ItemFn(ident, ..) | FnKind::Method(ident, ..)) = kind else {
        return None;
    };
    let span = cx.tcx.def_span(def_id);
    if span.from_expansion() {
        return None;
    }
    if test_code_exception && fn_in_test_code(cx, def_id) {
        return None;
    }
    Some(MeasuredFn {
        name: ident.name,
        span,
    })
}
