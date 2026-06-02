//! Recognising a `#[cfg(...)]` predicate that gates an item to test
//! builds, including a compound predicate like `cfg(all(test, unix))`
//! that `clippy_utils::is_cfg_test` — which only matches a bare
//! top-level `cfg(test)` — misses.

use rustc_hir::attrs::CfgEntry;
use rustc_hir::{HirId, find_attr};
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

/// Whether the item at `id` carries a `#[cfg(...)]` whose predicate
/// mentions `test` in a positive position — a bare `#[cfg(test)]` or a
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
pub(super) fn cfg_predicate_mentions_test(tcx: TyCtxt<'_>, id: HirId) -> bool {
    find_attr!(tcx, id, CfgTrace(cfgs) => cfgs)
        .is_some_and(|cfgs| cfgs.iter().any(|(cfg, _)| entry_mentions_test(cfg)))
}

/// Whether `test` appears as a predicate atom in a positive position.
/// `all(...)` and `any(...)` are searched recursively; `not(...)` is
/// deliberately *not* descended into. A `test` under a negation gates
/// the item *away* from test builds — the opposite of what the caller
/// asks — and an item whose only mention of `test` sits under a `not`
/// is configured out of a `cfg(test)` build entirely, so this rule
/// (which runs in that build) never sees it.
fn entry_mentions_test(cfg: &CfgEntry) -> bool {
    match cfg {
        CfgEntry::NameValue { name, .. } => *name == sym::test,
        CfgEntry::All(entries, _) | CfgEntry::Any(entries, _) => {
            entries.iter().any(entry_mentions_test)
        }
        CfgEntry::Not(..) | CfgEntry::Bool(..) | CfgEntry::Version(..) => false,
    }
}
