//! The locals a closure captures in a way a second closure could not
//! also hold.
//!
//! Every rule here that splits one closure into several has to ask this.
//! Two closures may both hold a shared borrow of a capture and no more
//! than that, so a capture held mutably, uniquely or by move is one the
//! split cannot hand to both halves: reaching it from two of them is
//! `E0499` where it is borrowed and `E0382` where it is moved.

use rustc_hir::HirId;
use rustc_hir::def_id::LocalDefId;
use rustc_lint::LateContext;
use rustc_middle::ty::{BorrowKind, CapturedPlace, UpvarCapture};

/// The locals `closure` captures any way but shared.
pub(crate) fn exclusive<'tcx>(cx: &LateContext<'tcx>, closure: LocalDefId) -> Vec<HirId> {
    cx.typeck_results()
        .closure_min_captures_flattened(closure)
        .filter(|capture| {
            !matches!(
                capture.info.capture_kind,
                UpvarCapture::ByRef(BorrowKind::Immutable),
            )
        })
        .map(CapturedPlace::get_root_variable)
        .collect()
}
