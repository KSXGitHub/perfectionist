//! The guard-and-value trigger: a closure passed to an `Option`-returning
//! adapter that welds a guard to a value, so one adapter does both.
//!
//! `filter_map` and its kin split on neither of the other triggers'
//! terms. Their closure returns an `Option`, so there is no conjunction
//! to cut, and a leading `map` would hand the next adapter an `Option`
//! rather than a value. What splits is the `Option` work inside: each
//! combinator has an iterator adapter that does the same thing, and the
//! split hands the work over.
//!
//! Which adapter it hands it to depends on the outer one's discipline,
//! which [`crate::adapter_discipline`] is about. A lifted `filter_map` in
//! front of a `map_while` would drop the very item that would have
//! stopped it, so a prefix-shaped adapter takes only the forms whose lift
//! target filters its way or not at all.

use super::combinator::{self, Yield};
use super::family::Family;
use super::{Finding, only_closure};
use crate::adapter_discipline::Discipline;
use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// The guard and value this adapter's closure holds.
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    method: Symbol,
    arguments: &'tcx [Expr<'tcx>],
    family: Family,
) -> Option<Finding> {
    if family != Family::Iterator {
        return None;
    }
    let (discipline, yields) = adapter(method)?;
    let closure = only_closure(arguments)?;
    let body = cx.tcx.hir_body(closure.body);
    let [parameter] = body.params else {
        return None;
    };
    let split = combinator::split(
        cx,
        body.value,
        closure.def_id,
        parameter.pat,
        discipline,
        yields,
    )?;
    Some(Finding {
        message: format!("this closure {}, so `{method}` does both", split.summary()),
        help: split.help(),
    })
}

/// The discipline of `method`'s answer and whether it yields a stream, or
/// `None` for a method this trigger does not speak about.
///
/// Yielding a stream is a second question the discipline does not answer.
/// `find_map` returns one value, so the only trailing adapter it has is
/// the `Option`'s own: a trailing `filter` there is `Option::filter`,
/// which compiles and can only reject what the search already settled on,
/// where the folded form kept looking.
fn adapter(method: Symbol) -> Option<(Discipline, Yield)> {
    Some(match method.as_str() {
        "filter_map" => (Discipline::Set, Yield::Stream),
        "find_map" => (Discipline::Set, Yield::OneValue),
        "map_while" => (Discipline::Prefix, Yield::Stream),
        _ => return None,
    })
}
