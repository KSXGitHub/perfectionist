use crate::module_reparse::{SpanRange, for_each_module_file};
use rustc_ast::visit::{self, Visitor};
use rustc_ast::{Expr, ExprKind, StmtKind};
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
        if let ExprKind::Closure(closure) = &expr.kind {
            let count = match &closure.body.kind {
                ExprKind::Block(block, _) => block
                    .stmts
                    .iter()
                    .filter(|statement| !matches!(statement.kind, StmtKind::Empty))
                    .count(),
                _ => 1,
            };
            self.counts.insert((expr.span.lo(), expr.span.hi()), count);
        }
        visit::walk_expr(self, expr);
    }
}
