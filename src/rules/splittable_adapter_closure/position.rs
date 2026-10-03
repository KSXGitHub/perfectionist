//! Whether the chain's place in the body is one the closure always
//! evaluates.
//!
//! A lifted step runs once per item the adapter pulls. A step left in
//! the closure runs once per item only where the closure always reaches
//! it, so hoisting one out of a conditional position changes behaviour.
//! That change compiles, which makes this the one gate in the rule whose
//! wrong answer is not a compile error:
//!
//! ```ignore
//! .map(|s| if flag { s.trim().parse::<usize>().unwrap() } else { 0 })
//! ```
//!
//! Hoisted, the `parse` runs for every item rather than for none, and
//! panics where the folded form returned `0`.
//!
//! The condition is sufficient rather than necessary. A step that
//! neither panics nor observes anything would hoist safely from a
//! conditional position, and this declines it, because Rust exposes no
//! purity test a lint could ask instead.

use rustc_hir::{BinOpKind, Expr, ExprKind, HirId, Node};
use rustc_lint::LateContext;

/// Whether every node between `chain` and the body root always
/// evaluates the child the chain came through.
pub(super) fn always_evaluated(cx: &LateContext<'_>, chain: HirId, body: HirId) -> bool {
    let mut child = chain;
    while child != body {
        match cx.tcx.parent_hir_node(child) {
            Node::Expr(parent) => {
                if !evaluates(parent, child) {
                    return false;
                }
                child = parent.hir_id;
            }
            // A statement, a `let` and the block holding them all run
            // when the block does, so which of them interposed does not
            // matter; the block's own position is what remains to ask.
            Node::Stmt(statement) => child = statement.hir_id,
            Node::LetStmt(local) => child = local.hir_id,
            Node::Block(block) => child = block.hir_id,
            _ => return false,
        }
    }
    true
}

/// Whether `parent` always evaluates the child `reached` through.
///
/// The shapes answering no are the ones that make evaluation
/// conditional: an arm of a `match` or an `if let`, which `?` desugars
/// into; the right operand of `&&` or `||`; and the body of a nested
/// closure or a loop, which may run any number of times including none.
/// Anything this does not recognise answers no as well.
fn evaluates(parent: &Expr<'_>, reached: HirId) -> bool {
    let is = |other: &Expr<'_>| other.hir_id == reached;
    match parent.kind {
        // The right operand of a short-circuiting operator is skipped
        // exactly where the left one settles the answer.
        ExprKind::Binary(operator, left, _) => {
            !matches!(operator.node, BinOpKind::And | BinOpKind::Or) || is(left)
        }
        // Each of these evaluates every operand it holds, a block's
        // statements and tail among them.
        ExprKind::Unary(..)
        | ExprKind::Cast(..)
        | ExprKind::Field(..)
        | ExprKind::AddrOf(..)
        | ExprKind::Index(..)
        | ExprKind::Tup(_)
        | ExprKind::Array(_)
        | ExprKind::Repeat(..)
        | ExprKind::Struct(..)
        | ExprKind::Assign(..)
        | ExprKind::AssignOp(..)
        | ExprKind::Call(..)
        | ExprKind::MethodCall(..)
        | ExprKind::Break(..)
        | ExprKind::Ret(_)
        | ExprKind::Become(_)
        | ExprKind::Block(..) => true,
        // The scrutinee runs; an arm does not. `?` lowers to a `match`
        // on its operand, so it needs no case of its own.
        ExprKind::Match(scrutinee, ..) => is(scrutinee),
        // The same split, where the condition takes the scrutinee's
        // place. An `if let` reaches here too, with its `let` for the
        // condition.
        ExprKind::If(condition, ..) => is(condition),
        _ => false,
    }
}
