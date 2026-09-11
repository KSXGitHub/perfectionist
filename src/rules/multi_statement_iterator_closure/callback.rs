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
    let trait_method = cx
        .tcx
        .opt_associated_item(method)
        .and_then(|item| item.trait_item_def_id())
        .unwrap_or(method);
    if !cx
        .tcx
        .is_diagnostic_item(sym::Iterator, cx.tcx.parent(trait_method))
    {
        return None;
    }
    let name = cx.tcx.item_name(trait_method);
    if !has_callback(name.as_str()) {
        return None;
    }
    Some((name, arguments.last()?))
}

fn has_callback(name: &str) -> bool {
    matches!(
        name,
        "map"
            | "filter"
            | "filter_map"
            | "flat_map"
            | "map_while"
            | "map_windows"
            | "scan"
            | "inspect"
            | "skip_while"
            | "take_while"
            | "for_each"
            | "try_for_each"
            | "fold"
            | "try_fold"
            | "reduce"
            | "try_reduce"
            | "find"
            | "find_map"
            | "try_find"
            | "any"
            | "all"
            | "position"
            | "rposition"
            | "partition"
            | "partition_in_place"
            | "is_partitioned"
            | "min_by"
            | "max_by"
            | "min_by_key"
            | "max_by_key"
            | "cmp_by"
            | "partial_cmp_by"
            | "eq_by",
    )
}
