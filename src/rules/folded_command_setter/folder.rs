//! Which setter the folder is, however the folder is written.
//!
//! Resolution is the trigger rather than spelling. `CommandExtra::with_arg`,
//! `Command::with_arg`, `<Command as CommandExtra>::with_arg` and any of
//! those reached through a renamed import are one method, so comparing
//! the resolved item makes every spelling fall out at once where an
//! enumeration would miss the ones nobody thought of.
//!
//! A closure needs its body walked first, because a closure that
//! forwards its parameters to the setter folds identically to the bare
//! path. What it must not do is compute on the way: the point of the
//! plural is that the closure disappears, and one that transforms its
//! item survives the rewrite as a `map` that was not there before.

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{
    BindingMode, BlockCheckMode, ByRef, Closure, Expr, ExprKind, HirId, Pat, PatKind, QPath,
};
use rustc_lint::LateContext;

/// The setter a folder resolves to, or `None` where the folder is
/// neither a path to one nor a closure forwarding to one.
///
/// A path bound to a local -- `let f = CommandExtra::without_env;` and
/// then `.fold(command, f)` -- folds exactly like the bare path but
/// resolves to the local, so it answers `None` here. A known gap rather
/// than a shape the rule means to exclude.
pub(super) fn resolves_to<'tcx>(cx: &LateContext<'tcx>, folder: &'tcx Expr<'tcx>) -> Option<DefId> {
    match folder.kind {
        ExprKind::Path(ref qpath) => assoc_fn(cx.qpath_res(qpath, folder.hir_id)),
        ExprKind::Closure(closure) => forwarded_by(cx, closure),
        _ => None,
    }
}

/// The associated function a resolution names, or `None` for anything
/// else -- a local, a unit struct, a free function.
fn assoc_fn(res: Res) -> Option<DefId> {
    match res {
        Res::Def(DefKind::AssocFn, def_id) => Some(def_id),
        _ => None,
    }
}

/// The setter a closure forwards to, or `None` where it does anything
/// besides forward.
fn forwarded_by<'tcx>(cx: &LateContext<'tcx>, closure: &'tcx Closure<'tcx>) -> Option<DefId> {
    let body = cx.tcx.hir_body(closure.body);
    let mut bound = Vec::new();
    for param in body.params {
        bindings_of(param.pat, &mut bound)?;
    }
    let (callee, arguments) = as_call(cx, unwrapped(body.value))?;
    if arguments.len() != bound.len() {
        return None;
    }
    if !arguments
        .iter()
        .zip(&bound)
        .all(|(argument, binding)| uses(argument, *binding))
    {
        return None;
    }
    Some(callee)
}

/// The bindings a parameter pattern introduces, in the order they were
/// bound, or `None` for a pattern the rewrite could not reproduce.
///
/// A tuple is walked into because the `with_env` pair needs it: its item
/// is a pair, so the closure destructures, and the bindings it yields
/// are what the setter is passed.
fn bindings_of(pat: &Pat<'_>, out: &mut Vec<HirId>) -> Option<()> {
    match pat.kind {
        // A `ref` binding hands the setter a reference to the item where
        // a by-value one hands it the item, so what the fold proves of
        // the item is not what the plural asks of it: `|c, ref k|
        // c.without_env(k)` establishes `&K: AsRef<OsStr>` and
        // `without_envs` wants `K: AsRef<OsStr>`. Measured as a
        // machine-applicable `E0277` on every release.
        PatKind::Binding(BindingMode(ByRef::No, _), hir_id, _, None) => out.push(hir_id),
        // A `..` would hide a field from the comparison below, leaving
        // the arity to agree by accident.
        PatKind::Tuple(elements, gap) if gap.as_opt_usize().is_none() => {
            for element in elements {
                bindings_of(element, out)?;
            }
        }
        _ => return None,
    }
    Some(())
}

/// `expr` with a block that only wraps one expression peeled off.
///
/// `|command, key| { command.without_env(key) }` forwards exactly as the
/// brace-less form does, and rustfmt leaves either as written, so the
/// two have to read alike. A block holding a statement is a different
/// matter: it is where a closure computes, which is what the plural
/// cannot absorb.
fn unwrapped<'tcx>(expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    let ExprKind::Block(block, None) = expr.kind else {
        return expr;
    };
    if block.rules != BlockCheckMode::DefaultBlock || !block.stmts.is_empty() {
        return expr;
    }
    match block.expr {
        Some(only) => unwrapped(only),
        None => expr,
    }
}

/// A call's callee and its arguments with the receiver first.
///
/// A method call and an associated-function call differ only in where
/// HIR puts the receiver -- `ExprKind::MethodCall` keeps it out of the
/// argument list, `ExprKind::Call` has it first -- so both are
/// normalised to receiver-then-arguments and compared once.
fn as_call<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(DefId, Vec<&'tcx Expr<'tcx>>)> {
    match expr.kind {
        ExprKind::MethodCall(_, receiver, arguments, _) => {
            let callee = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;
            let mut all = Vec::with_capacity(arguments.len() + 1);
            all.push(receiver);
            all.extend(arguments);
            Some((callee, all))
        }
        ExprKind::Call(callee, arguments) => {
            let ExprKind::Path(ref qpath) = callee.kind else {
                return None;
            };
            let def_id = assoc_fn(cx.qpath_res(qpath, callee.hir_id))?;
            Some((def_id, arguments.iter().collect()))
        }
        _ => None,
    }
}

/// Whether `expr` is a bare use of the binding `hir_id`, and so passes
/// it along untouched.
fn uses(expr: &Expr<'_>, hir_id: HirId) -> bool {
    let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind else {
        return false;
    };
    matches!(path.res, Res::Local(local) if local == hir_id)
}
