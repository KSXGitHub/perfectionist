use crate::module_reparse::{SpanRange, for_each_module_file};
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Block, Expr, ExprKind, StmtKind};
use rustc_lint::LateContext;
use std::collections::HashMap;

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
        if let Some(count) = closure_statements(expr) {
            self.counts.insert((expr.span.lo(), expr.span.hi()), count);
        }
        visit::walk_expr(self, expr);
    }
}

/// The top-level statements in the body of the closure `expr` is, or
/// that parentheses around it wrap; `None` where `expr` is neither.
///
/// Each pair of parentheses is keyed as well as the closure itself,
/// because HIR lowering drops `ExprKind::Paren` and widens the
/// closure's span to the parenthesised one. So `map((|value| ...))`
/// reaches the late pass carrying the span the parentheses cover, and
/// would miss a map keyed only by the span of the closure inside them.
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

/// The block a closure body is, or that a coroutine, `try` or `const`
/// wrapper around it holds; `None` for a body that is any other single
/// expression.
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
        ExprKind::Gen(_, block, _, _) => Some(block),
        ExprKind::TryBlock(block, _) => Some(block),
        ExprKind::ConstBlock(anonymous) => block_behind(&anonymous.value),
        _ => None,
    }
}
