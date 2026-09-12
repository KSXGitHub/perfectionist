use crate::common::{DefaultState, span_is_macro_generated};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_hir::{Expr, ExprKind, HirId, LangItem, MatchSource};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::Ty;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Symbol;
use std::collections::HashSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags one expression that builds a collection, walks it again,
    /// and builds another — `.collect::<HashSet<_>>().into_iter()
    /// .collect()`, `.collect::<Vec<_>>().into_iter().flatten()
    /// .collect()`, and the like.
    ///
    /// The first collection has to be one the chain assembled from a
    /// walk: a collection that merely arrives, as in
    /// `paths.unwrap_or_default().into_iter().map(PathBuf::from)
    /// .collect()`, is a conversion from one container to another and
    /// goes one way only.
    ///
    /// A `?` or an `.await` between two calls neither counts nor breaks
    /// the chain. A closure's body is measured on its own, so a chain
    /// inside a `map(|item| ...)` is a chain of its own. A chain
    /// produced by a macro expansion is not measured. Test code is
    /// measured like any other code; set `exempt_tests` to leave it
    /// alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// first collection is a value the code went to the trouble of
    /// building: it is usually the reason for the chain, and it is
    /// always something a reader can name. Walking straight back out of
    /// it hides that value in the middle of an expression, so the
    /// reader has to work out what was built and why before reading
    /// what became of it. Two computations in one expression also make
    /// the intermediate type invisible, and the intermediate type is
    /// what the first collection was chosen for — deduplication, or
    /// ordering, or lookup.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let mut unique: Vec<String> =
    ///     names.into_iter().collect::<HashSet<_>>().into_iter().collect();
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let deduplicated: HashSet<String> = names.into_iter().collect();
    /// let mut unique: Vec<String> = deduplicated.into_iter().collect();
    /// ```
    pub perfectionist::COLLECTION_ROUND_TRIP,
    Warn,
    "expression builds a collection, walks it again, and builds another",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::collection_round_trip";

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether test code is left alone: a chain inside a
    /// `#[cfg(test)]` module, a `#[test]` function, or an
    /// integration-test or benchmark target. Defaults to `false`, so a
    /// test is held to the same shape as the code it exercises.
    exempt_tests: bool,
}

pub struct CollectionRoundTrip {
    config: Config,
    /// Calls already measured as part of an outer chain, so one chain
    /// is reported once, from its head.
    measured: HashSet<HirId>,
}

impl_lint_pass!(CollectionRoundTrip => [COLLECTION_ROUND_TRIP]);

impl Register for rule::CollectionRoundTrip {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[COLLECTION_ROUND_TRIP]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(CollectionRoundTrip {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                measured: HashSet::new(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for CollectionRoundTrip {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if !matches!(expr.kind, ExprKind::MethodCall(..)) || self.measured.contains(&expr.hir_id) {
            return;
        }
        if span_is_macro_generated(expr.span) {
            return;
        }
        let stages = self.walk_spine(cx, expr);
        if !round_trips(&stages) {
            return;
        }
        if self.config.exempt_tests
            && item_in_test_code(cx, cx.tcx.hir_enclosing_body_owner(expr.hir_id))
        {
            return;
        }
        span_lint_and_help(
            cx,
            COLLECTION_ROUND_TRIP,
            expr.span,
            "expression builds a collection, walks it again, and builds another",
            None,
            "bind the first collection to a `let` named for what it holds: the walk that follows \
             is a second computation, and the type it walks is what the collection was chosen for",
        );
    }
}

/// What a call in the chain leaves behind, as far as this rule cares.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Collection,
    Iterator,
    Other,
}

/// Whether the chain builds a collection, iterates it, and collects
/// again. The walk has to come between the two collections: a chain
/// that ends at its first collection, or one that iterates before
/// building anything, goes one way only.
fn round_trips(stages: &[Stage]) -> bool {
    let mut built = false;
    let mut walked_after_building = false;
    for stage in stages {
        match stage {
            Stage::Collection if walked_after_building => return true,
            Stage::Collection => built = true,
            Stage::Iterator if built => walked_after_building = true,
            Stage::Iterator | Stage::Other => {}
        }
    }
    false
}

impl CollectionRoundTrip {
    /// The stages the calls from `head` down its receivers leave
    /// behind, in source order. Marks each call so the chain is not
    /// measured again from one of its own receivers.
    fn walk_spine<'tcx>(&mut self, cx: &LateContext<'tcx>, head: &'tcx Expr<'tcx>) -> Vec<Stage> {
        let mut stages = Vec::new();
        let mut current = head;
        loop {
            match current.kind {
                ExprKind::MethodCall(_, receiver, ..) if !span_is_macro_generated(current.span) => {
                    self.measured.insert(current.hir_id);
                    stages.push(stage_of(cx, current, receiver));
                    current = receiver;
                }
                // `receiver?` and `receiver.await` lower to a `match` on a
                // call wrapping the receiver; the chain runs through them.
                ExprKind::Match(
                    scrutinee,
                    _,
                    MatchSource::TryDesugar(_) | MatchSource::AwaitDesugar,
                ) => {
                    let ExprKind::Call(_, [inner]) = scrutinee.kind else {
                        break;
                    };
                    current = inner;
                }
                _ => break,
            }
        }
        stages.reverse();
        stages
    }
}

fn stage_of<'tcx>(cx: &LateContext<'tcx>, call: &Expr<'tcx>, receiver: &Expr<'tcx>) -> Stage {
    let produced = cx.typeck_results().expr_ty(call).peel_refs();
    let consumed = cx.typeck_results().expr_ty(receiver).peel_refs();
    if is_collection(cx, produced) && implements_lang_trait(cx, consumed, LangItem::Iterator) {
        Stage::Collection
    } else if implements_lang_trait(cx, produced, LangItem::Iterator) {
        Stage::Iterator
    } else {
        Stage::Other
    }
}

/// A container the chain has finished building. Text is not one of
/// these: a `String` flows through a chain the way a scalar does.
fn is_collection<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    const COLLECTIONS: &[Symbol] = &[
        sym::Vec,
        sym::VecDeque,
        sym::HashMap,
        sym::HashSet,
        sym::BTreeMap,
        sym::BTreeSet,
        sym::BinaryHeap,
    ];
    COLLECTIONS
        .iter()
        .any(|collection| is_diagnostic_type(cx, ty, *collection))
}

fn is_diagnostic_type<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>, name: Symbol) -> bool {
    ty.ty_adt_def()
        .is_some_and(|adt| cx.tcx.is_diagnostic_item(name, adt.did()))
}

fn implements_lang_trait<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>, item: LangItem) -> bool {
    cx.tcx
        .lang_items()
        .get(item)
        .is_some_and(|trait_id| implements_trait(cx, ty, trait_id, &[]))
}
