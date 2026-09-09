//! The cognitive-complexity walk over one function body.
//!
//! The metric is SonarSource's Cognitive Complexity, adapted to Rust
//! syntax. Every increment is one of three kinds:
//!
//! - **structural** — `+1`, plus one more for each level of nesting
//!   the construct sits at: `if`, `match`, `for`, `while`, `loop`;
//! - **hybrid** — `+1` with no nesting penalty, for a construct that
//!   continues a structure the reader is already inside: `else if`,
//!   `else`, `let ... else`, a match-arm guard;
//! - **fundamental** — `+1` with no nesting penalty: each run of like
//!   boolean operators (`a && b && c` is one run, `a && b || c` is
//!   two), each labelled `break` / `continue`, each recursive call.
//!
//! Nesting deepens inside the body of a structural construct and inside
//! a closure. A closure itself costs nothing.
//!
//! The walk measures what the author wrote. Code produced by a macro
//! expansion is opaque — it contributes nothing, though an expression
//! the author wrote as a macro argument is still counted. Compiler
//! desugarings are seen through: `?`, `.await`, and the `match` / `if`
//! that a `for` or `while` loop lowers to are not branches the reader
//! sees, so they add nothing beyond the loop's own increment.

use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    Arm, BinOpKind, Block, Body, ClosureKind, Expr, ExprKind, LetStmt, LoopSource, MatchSource,
};
use rustc_lint::LateContext;
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::Span;
use rustc_span::hygiene::ExpnKind;

/// The score of one function body, split so the diagnostic can say how
/// much of it is nesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Score {
    /// The whole cognitive complexity.
    pub total: usize,
    /// The part of `total` contributed by nesting penalties alone.
    pub from_nesting: usize,
}

/// Score the body of the function `fn_def_id`.
pub(super) fn score_body<'tcx>(
    cx: &LateContext<'tcx>,
    fn_def_id: LocalDefId,
    body: &'tcx Body<'tcx>,
) -> Score {
    let mut scorer = Scorer {
        tcx: cx.tcx,
        typeck: cx.tcx.typeck(fn_def_id),
        fn_def_id: fn_def_id.to_def_id(),
        nesting: 0,
        score: Score {
            total: 0,
            from_nesting: 0,
        },
        else_if: false,
        logical_parent: None,
    };
    scorer.visit_expr(body.value);
    scorer.score
}

struct Scorer<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    /// The function being scored, so a call back into it counts as
    /// recursion.
    fn_def_id: DefId,
    /// How many structural constructs and closures enclose the node
    /// being visited.
    nesting: usize,
    score: Score,
    /// Set by an `if` for the `if` in its `else` position, which is an
    /// `else if` and increments without a nesting penalty.
    else_if: bool,
    /// The boolean operator whose operand is being visited, so that a
    /// like operator directly beneath it continues the same run. Cleared
    /// on the way into any other kind of expression.
    logical_parent: Option<BinOpKind>,
}

impl<'tcx> Scorer<'tcx> {
    fn structural(&mut self) {
        self.score.total += 1 + self.nesting;
        self.score.from_nesting += self.nesting;
    }

    fn flat(&mut self) {
        self.score.total += 1;
    }

    fn nested(&mut self, visit: impl FnOnce(&mut Self)) {
        self.nesting += 1;
        visit(self);
        self.nesting -= 1;
    }

    fn visit_if(
        &mut self,
        expr: &'tcx Expr<'tcx>,
        cond: &'tcx Expr<'tcx>,
        then: &'tcx Expr<'tcx>,
        els: Option<&'tcx Expr<'tcx>>,
    ) {
        let is_else_if = core::mem::take(&mut self.else_if);
        // The `if` a `while` loop lowers to is the loop's, not a branch of
        // its own.
        if expr.span.desugaring_kind().is_some() {
            intravisit::walk_expr(self, expr);
            return;
        }
        if is_else_if {
            self.flat();
        } else {
            self.structural();
        }
        self.visit_expr(cond);
        self.nested(|scorer| scorer.visit_expr(then));
        let Some(els) = els else {
            return;
        };
        if matches!(els.kind, ExprKind::If(..)) && !is_macro_generated(els.span) {
            self.else_if = true;
            self.visit_expr(els);
        } else {
            self.flat();
            self.nested(|scorer| scorer.visit_expr(els));
        }
    }

    fn visit_match(&mut self, scrutinee: &'tcx Expr<'tcx>, arms: &'tcx [Arm<'tcx>]) {
        self.structural();
        self.visit_expr(scrutinee);
        self.nested(|scorer| {
            for arm in arms {
                if let Some(guard) = arm.guard {
                    scorer.flat();
                    scorer.visit_expr(guard);
                }
                scorer.visit_expr(arm.body);
            }
        });
    }

    fn visit_loop(&mut self, expr: &'tcx Expr<'tcx>, block: &'tcx Block<'tcx>, source: LoopSource) {
        // A `loop` with a desugaring span is the one `.await` lowers to;
        // `for` and `while` carry a desugaring span too but are the
        // author's loops.
        let is_authored = match source {
            LoopSource::Loop => expr.span.desugaring_kind().is_none(),
            LoopSource::While | LoopSource::ForLoop => true,
        };
        if !is_authored {
            intravisit::walk_expr(self, expr);
            return;
        }
        self.structural();
        self.nested(|scorer| scorer.visit_block(block));
    }

    fn visit_closure(&mut self, expr: &'tcx Expr<'tcx>, kind: ClosureKind) {
        // An `async` block or `async fn` body is a coroutine closure the
        // author never wrote as one, so it does not nest.
        let is_authored =
            matches!(kind, ClosureKind::Closure) && expr.span.desugaring_kind().is_none();
        if !is_authored {
            intravisit::walk_expr(self, expr);
            return;
        }
        self.nested(|scorer| intravisit::walk_expr(scorer, expr));
    }

    fn visit_logical(
        &mut self,
        op: BinOpKind,
        lhs: &'tcx Expr<'tcx>,
        rhs: &'tcx Expr<'tcx>,
        parent: Option<BinOpKind>,
    ) {
        if parent != Some(op) {
            self.flat();
        }
        self.logical_parent = Some(op);
        self.visit_expr(lhs);
        self.logical_parent = Some(op);
        self.visit_expr(rhs);
    }

    fn is_recursive_call(&self, expr: &Expr<'_>) -> bool {
        match expr.kind {
            ExprKind::Call(callee, _) => {
                let ExprKind::Path(qpath) = &callee.kind else {
                    return false;
                };
                self.typeck.qpath_res(qpath, callee.hir_id).opt_def_id() == Some(self.fn_def_id)
            }
            ExprKind::MethodCall(..) => {
                self.typeck.type_dependent_def_id(expr.hir_id) == Some(self.fn_def_id)
            }
            _ => false,
        }
    }
}

impl<'tcx> Visitor<'tcx> for Scorer<'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        let logical_parent = self.logical_parent.take();
        if is_macro_generated(expr.span) {
            self.else_if = false;
            intravisit::walk_expr(self, expr);
            return;
        }
        match expr.kind {
            ExprKind::If(cond, then, els) => self.visit_if(expr, cond, then, els),
            ExprKind::Match(scrutinee, arms, MatchSource::Normal | MatchSource::Postfix) => {
                self.visit_match(scrutinee, arms);
            }
            ExprKind::Loop(block, _, source, _) => self.visit_loop(expr, block, source),
            ExprKind::Closure(closure) => self.visit_closure(expr, closure.kind),
            ExprKind::Binary(op, lhs, rhs) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
                self.visit_logical(op.node, lhs, rhs, logical_parent);
            }
            ExprKind::Break(destination, _) | ExprKind::Continue(destination) => {
                if destination.label.is_some() {
                    self.flat();
                }
                intravisit::walk_expr(self, expr);
            }
            ExprKind::Call(..) | ExprKind::MethodCall(..) => {
                if self.is_recursive_call(expr) {
                    self.flat();
                }
                intravisit::walk_expr(self, expr);
            }
            _ => intravisit::walk_expr(self, expr),
        }
    }

    fn visit_local(&mut self, local: &'tcx LetStmt<'tcx>) {
        if let Some(init) = local.init {
            self.visit_expr(init);
        }
        let Some(els) = local.els else {
            return;
        };
        if !is_macro_generated(local.span) {
            self.flat();
        }
        self.nested(|scorer| scorer.visit_block(els));
    }
}

/// Whether `span` was produced by a macro expansion — a `macro_rules!`
/// or proc macro, from this crate or another — as opposed to written
/// by the author or produced by a compiler desugaring.
fn is_macro_generated(span: Span) -> bool {
    span.macro_backtrace()
        .any(|expansion| matches!(expansion.kind, ExpnKind::Macro(..)))
}
