//! Recognising code that exists only in a test build.
//!
//! Two questions live here, and rules ask them for different
//! reasons. [`cfg_predicate_implies_test`] answers "does *this* item's
//! `#[cfg(...)]` gate it to test builds?", including a compound
//! predicate like `cfg(all(test, unix))` that
//! `clippy_utils::is_cfg_test` — which only matches a bare top-level
//! `cfg(test)` — misses. [`in_test_code`] answers the enclosing-scope
//! version: is the node itself, or anything lexically containing it,
//! so gated — or is it inside a `#[test]` function?
//!
//! Both only ever see test code in a build where `cfg(test)` is
//! active: `#[cfg(test)]` items are configured out before a late pass
//! runs otherwise. That build is the unit-test target
//! `cargo dylint -- --all-targets` adds.

use crate::cargo_target::crate_target;
use clippy_utils::is_in_test_function;
use core::iter::once;
use rustc_hir::attrs::CfgEntry;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, find_attr};
use rustc_lint::LateContext;
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

/// Whether the node at `id` is test-exclusive code: it or a lexical
/// ancestor carries a `#[cfg(...)]` that implies `test` (see
/// [`cfg_predicate_implies_test`]), or it sits inside a `#[test]`
/// function.
///
/// The `#[test]` half is `clippy_utils::is_in_test_function`, which
/// matches the *enclosing* function by name against the test
/// descriptors rustc synthesises in the surrounding module. Only a
/// function rustc itself registered as a test matches, and one of
/// those takes no parameters — so for a parameter-shaped rule this
/// half fires only on an item nested inside a test body, a helper
/// `fn` declared in the test function's own block. A test written
/// with a third-party attribute is *not* matched by it and reaches
/// the `cfg` half instead; see "Third-party test attributes" in
/// `planned-rules/IMPLEMENTATION_CONVENTIONS.md`.
///
/// The `cfg` half checks `id` itself as well as its ancestors, unlike
/// `clippy_utils::is_in_cfg_test`, which starts at the parent: a
/// `#[cfg(test)] fn` is test code just as much as a `fn` inside
/// `#[cfg(test)] mod tests`.
pub(crate) fn in_test_code(tcx: TyCtxt<'_>, id: HirId) -> bool {
    is_in_test_function(tcx, id)
        || once(id)
            .chain(tcx.hir_parent_id_iter(id))
            .any(|ancestor| cfg_predicate_implies_test(tcx, ancestor))
}

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
/// after configuration and walks every `all(...)` / `any(...)` /
/// `not(...)` nesting.
pub(crate) fn cfg_predicate_implies_test(tcx: TyCtxt<'_>, id: HirId) -> bool {
    find_attr!(tcx, id, CfgTrace(cfgs) => cfgs)
        .is_some_and(|cfgs| cfgs.iter().any(|(cfg, _)| entry_implies_test(cfg, false)))
}

/// Whether the predicate `cfg`, read under a negation when `negated`,
/// holds *only* in a `test` build — i.e. whether it implies `test`.
///
/// `all(...)` and `any(...)` compose the implication differently:
///
/// - `all(e1, ..., en)` implies `test` ⟸ **some** `ei` implies `test`;
///   the others only further restrict when the item compiles.
/// - `any(e1, ..., en)` implies `test` ⟸ **every** `ei` implies `test`.
///   If even one branch can hold without `test` (e.g. `any(test, unix)`
///   on a `unix` build), the item is real production code, not
///   test-only.
///
/// This composes through nesting — e.g. `all(any(test, foo), bar)` is
/// *not* recognised as test-only, because `any(test, foo)` is not.
///
/// `not(...)` flips `negated` rather than recursing into a rewritten
/// predicate, which is De Morgan applied on the fly: under a negation
/// an `all` behaves like an `any` and vice versa, and a bare `test`
/// gates the item *away* from test builds. So `not(not(test))` is
/// recognised as test-only, while `not(test)` is not — and neither
/// costs more than a linear walk. What the walk deliberately does not
/// decide is under *Completeness* below.
///
/// # Completeness
///
/// The answers above are *sound but incomplete*: a `true` is always
/// right, a `false` may be a missed recognition. Deciding
/// `P → test` in general is deciding unsatisfiability of
/// `P ∧ ¬test`, i.e. co-NP-complete, so a complete answer is a SAT
/// solver over the predicate's atoms. What this walk computes instead
/// is the negation-normal-form reading, which agrees with the complete
/// answer on every predicate whose atoms are independent — which is to
/// say on every `cfg` anyone writes. The shapes it gives up on need a
/// contradiction or a tautology spelled out across branches
/// (`any(test, all(a, not(a)))`), and are not worth a solver.
fn entry_implies_test(cfg: &CfgEntry, negated: bool) -> bool {
    match cfg {
        CfgEntry::NameValue { name, .. } => !negated && *name == sym::test,
        CfgEntry::All(entries, _) if !negated => {
            entries.iter().any(|entry| entry_implies_test(entry, false))
        }
        CfgEntry::All(entries, _) => entries.iter().all(|entry| entry_implies_test(entry, true)),
        CfgEntry::Any(entries, _) if !negated => {
            entries.iter().all(|entry| entry_implies_test(entry, false))
        }
        CfgEntry::Any(entries, _) => entries.iter().any(|entry| entry_implies_test(entry, true)),
        CfgEntry::Not(entry, _) => entry_implies_test(entry, !negated),
        CfgEntry::Bool(..) | CfgEntry::Version(..) => false,
    }
}

/// Whether the function `fn_def_id` is test code under either reading:
/// the whole crate is an integration-test or benchmark target, or the
/// function sits in test-exclusive code per [`in_test_code`].
pub(crate) fn fn_in_test_code(cx: &LateContext<'_>, fn_def_id: LocalDefId) -> bool {
    crate_target(cx).is_test_target()
        || in_test_code(cx.tcx, cx.tcx.local_def_id_to_hir_id(fn_def_id))
}
