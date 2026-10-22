use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::{Symbol, sym};

pub(super) fn iterator_callback<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(Symbol, &'tcx Expr<'tcx>)> {
    let (method, arguments) = match expr.kind {
        ExprKind::MethodCall(_, _, arguments, _) => (
            cx.typeck_results().type_dependent_def_id(expr.hir_id)?,
            arguments,
        ),
        ExprKind::Call(callee, arguments) => {
            let ExprKind::Path(path) = &callee.kind else {
                return None;
            };
            (
                cx.typeck_results()
                    .qpath_res(path, callee.hir_id)
                    .opt_def_id()?,
                arguments,
            )
        }
        _ => return None,
    };
    let declaring = cx.tcx.trait_of_assoc(method)?;
    if !cx.tcx.is_diagnostic_item(sym::Iterator, declaring) {
        return None;
    }
    let name = cx.tcx.item_name(method);
    if !has_callback(name.as_str()) {
        return None;
    }
    Some((name, arguments.last()?))
}

/// Every method on the standard `Iterator` trait that takes a
/// callback, grouped by what the callback does.
pub(super) const CALLBACK_METHODS: &[&str] = &[
    // Transforming.
    "map",
    "filter_map",
    "flat_map",
    "map_while",
    "map_windows",
    "scan",
    // Filtering.
    "filter",
    "skip_while",
    "take_while",
    // Visiting.
    "inspect",
    "for_each",
    "try_for_each",
    // Generating.
    "intersperse_with",
    // Reducing.
    "fold",
    "try_fold",
    "reduce",
    "try_reduce",
    // Searching.
    "find",
    "find_map",
    "try_find",
    "position",
    "rposition",
    "any",
    "all",
    // Partitioning.
    "partition",
    "partition_in_place",
    "is_partitioned",
    // Comparing.
    "min_by",
    "max_by",
    "min_by_key",
    "max_by_key",
    "cmp_by",
    "partial_cmp_by",
    "eq_by",
    "is_sorted_by",
    "is_sorted_by_key",
];

fn has_callback(name: &str) -> bool {
    CALLBACK_METHODS.contains(&name)
}
