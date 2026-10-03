//! The locals a closure captures in a way a second closure could not
//! also hold.
//!
//! Every rule here that splits one closure into several has to ask this.
//! Two closures may both hold a shared borrow of a capture, and both
//! hold their own copy of a `Copy` one, and no more than that, so a
//! capture held mutably or uniquely, or moved and not `Copy`, is one the
//! split cannot hand to both halves: reaching it from two of them is
//! `E0499` where it is borrowed and `E0382` where it is moved.
//!
//! The `Copy` clause is what keeps a `move` closure answerable, since
//! `move` captures everything by value whether or not the body needs it
//! that way. It asks that the closure not write to the capture: two
//! closures holding their own copies of one the body increments see
//! different values, which compiles and answers differently.

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Mutability};
use rustc_lint::LateContext;
use rustc_middle::ty::{BorrowKind, CapturedPlace, UpvarCapture};

/// The locals `closure` captures in a way a second closure could not
/// also hold.
pub(crate) fn exclusive<'tcx>(cx: &LateContext<'tcx>, closure: LocalDefId) -> Vec<HirId> {
    cx.typeck_results()
        .closure_min_captures_flattened(closure)
        .filter(|capture| match capture.info.capture_kind {
            UpvarCapture::ByRef(BorrowKind::Immutable) => false,
            UpvarCapture::ByValue => {
                capture.mutability == Mutability::Mut
                    || !cx.type_is_copy_modulo_regions(capture.place.ty())
            }
            _ => true,
        })
        .map(CapturedPlace::get_root_variable)
        .collect()
}
