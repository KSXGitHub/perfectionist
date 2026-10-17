use super::callback::CALLBACK_METHODS;
use crate::module_reparse::{SpanRange, for_each_module_file};
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Block, Expr, ExprKind, StmtKind};
use rustc_lint::LateContext;
use rustc_span::Symbol;
use std::collections::HashMap;

/// The top-level statement count of every callback the author wrote,
/// keyed by the byte range that callback occupies.
///
/// A range is recorded only where the source itself shows a closure
/// passed as the final argument of a call to one of
/// [`CALLBACK_METHODS`]. That is what makes a lookup the whole
/// verification rather than half of one: a closure the late pass finds
/// in callback position is the author's own only if the bytes under it
/// are a callback the author wrote. An expansion cannot arrange that by
/// borrowing a span, because the span it borrows has to cover a closure
/// the author already passed to one of these methods.
pub(super) type StatementCounts = HashMap<SpanRange, usize>;

pub(super) fn count_source_statements(cx: &LateContext<'_>) -> StatementCounts {
    let mut counts = StatementCounts::new();
    for_each_module_file(cx, |krate| {
        visit::walk_crate(
            &mut CountStatements {
                counts: &mut counts,
            },
            krate,
        );
    });
    counts
}

struct CountStatements<'a> {
    counts: &'a mut StatementCounts,
}

impl<'ast> Visitor<'ast> for CountStatements<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Some((method, argument)) = final_argument(expr)
            && CALLBACK_METHODS.contains(&method.as_str())
            && let Some(count) = closure_statements(argument)
        {
            // The argument's own span is the only key the late pass
            // ever looks up, parentheses included: lowering drops
            // `ExprKind::Paren` and widens the closure's span to the
            // outermost parenthesised one, which is the span of the
            // argument as written. Keying the levels inside it as well
            // was measured to change nothing.
            self.counts
                .insert((argument.span.lo(), argument.span.hi()), count);
        }
        visit::walk_expr(self, expr);
    }
}

/// The method name the call `expr` is written with, paired with its
/// final argument; `None` where `expr` is not a call or carries no
/// arguments.
///
/// The name is the one in the source rather than the one the call
/// resolves to. Asking the source is the point: a macro cannot take an
/// author-written call of some other function and have it answer for an
/// `Iterator` method.
fn final_argument(expr: &Expr) -> Option<(Symbol, &Expr)> {
    match &expr.kind {
        ExprKind::MethodCall(call) => Some((call.seg.ident.name, &**call.args.last()?)),
        ExprKind::Call(callee, arguments) => {
            let ExprKind::Path(_, path) = &callee.kind else {
                return None;
            };
            Some((path.segments.last()?.ident.name, &**arguments.last()?))
        }
        _ => None,
    }
}

/// The top-level statements in the body of the closure `expr` is, or
/// that parentheses around it wrap; `None` where `expr` is neither.
fn closure_statements(expr: &Expr) -> Option<usize> {
    match &expr.kind {
        ExprKind::Closure(closure) => Some(match block_behind(&closure.body) {
            Some(block) => block
                .stmts
                .iter()
                .filter(|statement| !matches!(statement.kind, StmtKind::Empty))
                .count(),
            None => 1,
        }),
        ExprKind::Paren(inner) => closure_statements(inner),
        _ => None,
    }
}

/// The block a closure body is, or that parentheses or a coroutine,
/// `try` or `const` wrapper around it hold; `None` for a body that is
/// any other single expression.
///
/// A wrapped block is counted through, as a plain or `unsafe` one is:
/// the statements are the steps the callback performs either way, and
/// `|handle| async move { ... }` is how a callback performs them in an
/// asynchronous pipeline. Counting only `ExprKind::Block` would draw
/// the line at which variant the parser picked rather than at anything
/// a reader of the callback can see.
fn block_behind(body: &Expr) -> Option<&Block> {
    match &body.kind {
        ExprKind::Block(block, _) => Some(block),
        ExprKind::Paren(inner) => block_behind(inner),
        ExprKind::Gen(_, block, _, _) => Some(block),
        ExprKind::TryBlock(block, _) => Some(block),
        ExprKind::ConstBlock(anonymous) => block_behind(&anonymous.value),
        _ => None,
    }
}
