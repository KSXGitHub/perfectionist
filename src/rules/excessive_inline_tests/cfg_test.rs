//! Recognising a `#[cfg(...)]` predicate that gates an item to test
//! builds, including a compound predicate like `cfg(all(test, unix))`
//! that `clippy_utils::is_cfg_test` — which only matches a bare
//! top-level `cfg(test)` — misses.

use rustc_hir::attrs::CfgEntry;
use rustc_hir::{HirId, find_attr};
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

/// Whether the item at `id` carries a `#[cfg(...)]` whose predicate is
/// satisfiable *only* when `test` is set — a bare `#[cfg(test)]` or a
/// compound `#[cfg(all(test, unix))]`, `#[cfg(all(test, feature =
/// "..."))]`, and so on.
///
/// `clippy_utils::is_cfg_test` only recognises a top-level `test`
/// entry, so a `mod tests;` gated by a compound predicate is otherwise
/// misclassified as a production module — leaving its target file to be
/// re-flagged as inline test code living in a production file (the
/// false positive of <https://github.com/KSXGitHub/perfectionist/issues/187>).
/// This reads the parsed `CfgTrace` predicate rustc leaves on the item
/// after configuration and walks every `all(...)` / `any(...)` nesting.
pub(super) fn cfg_predicate_implies_test(tcx: TyCtxt<'_>, id: HirId) -> bool {
    find_attr!(tcx, id, CfgTrace(cfgs) => cfgs)
        .is_some_and(|cfgs| cfgs.iter().any(|(cfg, _)| entry_implies_test(cfg)))
}

/// Whether the predicate `cfg` holds *only* in a `test` build — i.e. it
/// implies `test`. `not(...)` is deliberately *not* descended into: a
/// `test` under a negation gates the item *away* from test builds — the
/// opposite of what the caller asks — and an item whose only mention of
/// `test` sits under a `not` is configured out of a `cfg(test)` build
/// entirely, so this rule (which runs in that build) never sees it.
///
/// `all(...)` and `any(...)` compose the implication differently:
///
/// - `all(e1, ..., en)` implies `test` ⟺ **some** `ei` implies `test`;
///   the others only further restrict when the item compiles.
/// - `any(e1, ..., en)` implies `test` ⟺ **every** `ei` implies `test`.
///   If even one branch can hold without `test` (e.g. `any(test, unix)`
///   on a `unix` build), the item is real production code, not
///   test-only.
///
/// This composes correctly through nesting — e.g. `all(any(test, foo),
/// bar)` is *not* test-only, because `any(test, foo)` is not.
fn entry_implies_test(cfg: &CfgEntry) -> bool {
    match cfg {
        CfgEntry::NameValue { name, .. } => *name == sym::test,
        CfgEntry::All(entries, _) => entries.iter().any(entry_implies_test),
        CfgEntry::Any(entries, _) => entries.iter().all(entry_implies_test),
        CfgEntry::Not(..) | CfgEntry::Bool(..) | CfgEntry::Version(..) => false,
    }
}
