//! The nesting walk over one function body.
//!
//! A construct nests when a reader indents for what is inside it: an
//! `if` (with its `else if` arms at the same level and each `else`
//! body inside), a `match` and the arms inside it, a `for` / `while` /
//! `loop`, a closure, a `let ... else` body, and a free-standing block —
//! a statement block, an `unsafe` block, or the block a `let`
//! initialises from. The block that *is* a construct's body does not
//! nest again on its own: `if x { y }` is one level, not two.
//!
//! The walk measures what the author wrote. A construct produced by a
//! macro expansion adds no level, though the author's constructs inside
//! a macro's arguments still count, and the desugared shape of a `for`
//! or `while` loop, a `?`, an `.await`, or an `async` body adds nothing
//! beyond the construct the author wrote.

use crate::common::span_is_macro_generated;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Arm, Block, Body, ClosureKind, Expr, ExprKind, LetStmt, LoopSource, MatchSource};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

/// The deepest point of a body: how many constructs enclose it and the
/// span of the construct at that depth.
#[derive(Debug, Clone, Copy)]
pub(super) struct Deepest {
    pub depth: usize,
    pub span: Span,
}

/// The deepest nesting in `body`, or `None` when nothing in it nests.
pub(super) fn deepest_nesting<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>) -> Option<Deepest> {
    let mut walker = Walker {
        tcx,
        depth: 0,
        deepest: None,
        else_if: false,
    };
    walker.visit_body_expr(body.value);
    walker.deepest
}

struct Walker<'tcx> {
    tcx: TyCtxt<'tcx>,
    /// How many constructs enclose the node being visited.
    depth: usize,
    deepest: Option<Deepest>,
    /// Set by an `if` for the `if` in its `else` position, which is an
    /// `else if` and stays at the outer `if`'s level.
    else_if: bool,
}

impl<'tcx> Walker<'tcx> {
    /// Visit the inside of a construct one level deeper.
    fn enter(&mut self, span: Span, visit: impl FnOnce(&mut Self)) {
        self.depth += 1;
        if self
            .deepest
            .is_none_or(|deepest| self.depth > deepest.depth)
        {
            self.deepest = Some(Deepest {
                depth: self.depth,
                span,
            });
        }
        visit(self);
        self.depth -= 1;
    }

    /// Visit an expression in body position — the `then` of an `if`, an
    /// arm's body, a closure's body — where a block is the construct's
    /// own body rather than a nested one.
    fn visit_body_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Block(block, None) = expr.kind else {
            self.visit_expr(expr);
            return;
        };
        for stmt in block.stmts {
            self.visit_stmt(stmt);
        }
        if let Some(tail) = block.expr {
            self.visit_expr(tail);
        }
    }

    fn visit_if(
        &mut self,
        expr: &'tcx Expr<'tcx>,
        cond: &'tcx Expr<'tcx>,
        then: &'tcx Expr<'tcx>,
        els: Option<&'tcx Expr<'tcx>>,
    ) {
        let is_else_if = core::mem::take(&mut self.else_if);
        self.visit_expr(cond);
        // The `if` a `while` loop lowers to is the loop's, not a level of
        // its own.
        if expr.span.desugaring_kind().is_some() {
            self.visit_body_expr(then);
            if let Some(els) = els {
                self.visit_body_expr(els);
            }
            return;
        }
        let visit_branches = |walker: &mut Self| {
            walker.visit_body_expr(then);
            let Some(els) = els else {
                return;
            };
            if matches!(els.kind, ExprKind::If(..)) && !span_is_macro_generated(els.span) {
                walker.else_if = true;
                walker.visit_expr(els);
            } else {
                walker.visit_body_expr(els);
            }
        };
        if is_else_if {
            visit_branches(self);
        } else {
            self.enter(expr.span, visit_branches);
        }
    }

    fn visit_match(
        &mut self,
        expr: &'tcx Expr<'tcx>,
        scrutinee: &'tcx Expr<'tcx>,
        arms: &'tcx [Arm<'tcx>],
        source: MatchSource,
    ) {
        self.visit_expr(scrutinee);
        let visit_arms = |walker: &mut Self| {
            for arm in arms {
                if let Some(guard) = arm.guard {
                    walker.visit_expr(guard);
                }
                walker.visit_body_expr(arm.body);
            }
        };
        if matches!(source, MatchSource::Normal | MatchSource::Postfix) {
            self.enter(expr.span, visit_arms);
        } else {
            visit_arms(self);
        }
    }

    fn visit_loop(&mut self, expr: &'tcx Expr<'tcx>, block: &'tcx Block<'tcx>, source: LoopSource) {
        // A `loop` with a desugaring span is the one `.await` lowers to;
        // `for` and `while` carry a desugaring span too but are the
        // author's loops.
        let is_authored = match source {
            LoopSource::Loop => expr.span.desugaring_kind().is_none(),
            LoopSource::While | LoopSource::ForLoop => true,
        };
        if is_authored {
            self.enter(expr.span, |walker| intravisit::walk_block(walker, block));
        } else {
            intravisit::walk_block(self, block);
        }
    }

    fn visit_closure(&mut self, expr: &'tcx Expr<'tcx>, kind: ClosureKind, body: &'tcx Body<'tcx>) {
        // An `async` block or `async fn` body is a coroutine closure the
        // author never wrote as one, so it is not a level.
        let is_authored =
            matches!(kind, ClosureKind::Closure) && expr.span.desugaring_kind().is_none();
        if is_authored {
            self.enter(expr.span, |walker| walker.visit_body_expr(body.value));
        } else {
            self.visit_body_expr(body.value);
        }
    }
}

impl<'tcx> Visitor<'tcx> for Walker<'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if span_is_macro_generated(expr.span) {
            self.else_if = false;
            intravisit::walk_expr(self, expr);
            return;
        }
        match expr.kind {
            ExprKind::If(cond, then, els) => self.visit_if(expr, cond, then, els),
            ExprKind::Match(scrutinee, arms, source) => {
                self.visit_match(expr, scrutinee, arms, source);
            }
            ExprKind::Loop(block, _, source, _) => self.visit_loop(expr, block, source),
            ExprKind::Closure(closure) => {
                let body = self.tcx.hir_body(closure.body);
                self.visit_closure(expr, closure.kind, body);
            }
            // A block the compiler made — the `unsafe` block an `.await`
            // polls in — is not one the reader indents for.
            ExprKind::Block(block, _) if expr.span.desugaring_kind().is_some() => {
                intravisit::walk_block(self, block);
            }
            ExprKind::Block(block, _) => {
                self.enter(expr.span, |walker| intravisit::walk_block(walker, block));
            }
            // The wrapper an `async fn` body lowers into; the body inside it
            // is the function's own.
            ExprKind::DropTemps(inner) => self.visit_body_expr(inner),
            _ => intravisit::walk_expr(self, expr),
        }
    }

    fn visit_local(&mut self, local: &'tcx LetStmt<'tcx>) {
        if let Some(init) = local.init {
            self.visit_expr(init);
        }
        if let Some(els) = local.els {
            self.enter(els.span, |walker| intravisit::walk_block(walker, els));
        }
    }
}
