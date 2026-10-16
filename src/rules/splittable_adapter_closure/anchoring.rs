//! Whether the chain's place in the body is one the closure always
//! evaluates.
//!
//! A lifted step runs once per item the adapter pulls. A step left in
//! the closure runs once per item only where the closure always reaches
//! it, so hoisting one out of a conditional position changes behaviour.
//! That change compiles, which makes this the one gate in the chain
//! trigger whose wrong answer is not a compile error:
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

use rustc_hir::intravisit::{Visitor, walk_expr};
use rustc_hir::{
    AssignOpKind, BinOpKind, Expr, ExprKind, HirId, MatchSource, Node, Stmt, StmtKind,
};
use rustc_lint::LateContext;
use rustc_span::Spanned;

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
            // A `let`'s `else` block is the exception: it runs only
            // where the pattern does not match.
            Node::LetStmt(local) if local.els.is_some_and(|els| els.hir_id == child) => {
                return false;
            }
            Node::LetStmt(local) => child = local.hir_id,
            Node::Block(block) => child = block.hir_id,
            // A struct expression's field is a node of its own, and
            // every field it holds is evaluated.
            Node::ExprField(field) => child = field.hir_id,
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
        // place.
        ExprKind::If(condition, ..) => is(condition),
        _ => false,
    }
}

/// Whether anything evaluated before `chain` can leave the closure.
///
/// [`always_evaluated`] asks what stands above the chain, which says
/// nothing about what runs before it. Anything reached first that leaves
/// the closure leaves the chain unevaluated, so a lifted step would run
/// for every item where the folded form ran it for none. The same program
/// written as an `if` whose branch holds the chain is declined by the
/// walk, and these two answers have to agree.
///
/// What runs first is read from the tree rather than from span order,
/// because a span answers the question only where both spans are in one
/// file: a `panic!` expands in `core`, so its byte positions live in
/// another file's range of the `SourceMap` and comparing them with the
/// chain's answers nothing. And one position in Rust evaluates against
/// source order anyway: an assignment's right side runs before the place
/// it is assigned to.
///
/// What leaves is read from the type: an expression of type `!` does not
/// return, which covers a `panic!`, a `std::process::exit`, a call to a
/// `-> !` function and a `loop {}` as readily as a `return` or a `break`.
/// A `?` is the one that leaves while typing as its output, so it keeps a
/// case of its own.
///
/// A nested closure's own `return` leaves that closure rather than this
/// one, which is why the visitor stays at its default nesting filter.
pub(super) fn nothing_observable_first<'tcx>(
    cx: &LateContext<'tcx>,
    chain: HirId,
    body: HirId,
) -> bool {
    let mut child = chain;
    let mut earlier: Vec<&'tcx Expr<'tcx>> = Vec::new();
    while child != body {
        match cx.tcx.parent_hir_node(child) {
            Node::Expr(parent) => {
                preceding(parent, child, &mut earlier);
                child = parent.hir_id;
            }
            Node::Stmt(statement) => child = statement.hir_id,
            Node::LetStmt(local) => child = local.hir_id,
            Node::ExprField(field) => child = field.hir_id,
            Node::Block(block) => {
                // Every statement before the one the chain is in runs
                // first, and so does nothing after it.
                let reached = block
                    .stmts
                    .iter()
                    .position(|statement| statement.hir_id == child);
                let before = &block.stmts[..reached.unwrap_or(block.stmts.len())];
                // A `let`-`else` leaves the closure exactly where its
                // pattern does not match, and its `else` block is
                // divergent by construction, so nothing has to be read
                // from a type for it.
                if before.iter().any(|statement| leaves(statement)) {
                    return false;
                }
                earlier.extend(statements(before));
                child = block.hir_id;
            }
            // The walk above has already declined anything else.
            _ => return false,
        }
    }
    !earlier.iter().any(|expression| observable(cx, expression))
}

/// Whether `statement` is a `let`-`else`, whose `else` block leaves the
/// closure where the pattern does not match.
fn leaves(statement: &Stmt<'_>) -> bool {
    matches!(statement.kind, StmtKind::Let(local) if local.els.is_some())
}

/// The expressions `parent` evaluates before the child the chain came
/// through.
fn preceding<'tcx>(parent: &'tcx Expr<'tcx>, reached: HirId, earlier: &mut Vec<&'tcx Expr<'tcx>>) {
    // Each arm below answers for the positions that are not operands
    // before it reaches here, so the fallback is unreachable; counting
    // everything as preceding rather than nothing is what makes an arm
    // added without one err toward declining.
    let before = |operands: &'tcx [Expr<'tcx>]| {
        let reached_at = operands
            .iter()
            .position(|operand| operand.hir_id == reached);
        &operands[..reached_at.unwrap_or(operands.len())]
    };
    match parent.kind {
        // The right side runs before the place it is assigned to, which
        // is the one position where evaluation order and source order
        // disagree.
        ExprKind::Assign(_, value, _) => {
            if reached != value.hir_id {
                earlier.push(value);
            }
        }
        // A compound assignment's order is the plain one's for primitives
        // and the reverse for an overloaded operator, so either operand
        // counts as the one that may have run first.
        ExprKind::AssignOp(_, place, value) => {
            earlier.push(match reached == value.hir_id {
                true => place,
                false => value,
            });
        }
        ExprKind::Binary(_, left, right) => {
            if reached == right.hir_id {
                earlier.push(left);
            }
        }
        ExprKind::Index(base, index, _) => {
            if reached == index.hir_id {
                earlier.push(base);
            }
        }
        ExprKind::MethodCall(_, receiver, arguments, _) => {
            if reached != receiver.hir_id {
                earlier.push(receiver);
                earlier.extend(before(arguments));
            }
        }
        ExprKind::Call(callee, arguments) => {
            if reached != callee.hir_id {
                earlier.push(callee);
                earlier.extend(before(arguments));
            }
        }
        ExprKind::Tup(operands) | ExprKind::Array(operands) => {
            earlier.extend(before(operands));
        }
        // A field's expression is reached through the field's own node,
        // and the `..base` is reached as an expression, so a search that
        // finds neither means the base, which every field precedes.
        ExprKind::Struct(_, fields, _) => {
            let reached_at = fields.iter().position(|field| field.hir_id == reached);
            let before = &fields[..reached_at.unwrap_or(fields.len())];
            earlier.extend(before.iter().map(|field| field.expr));
        }
        // Everything else the walk admits holds one operand, or holds the
        // chain in a position it has already answered for.
        _ => {}
    }
}

/// The expressions a run of statements evaluates.
fn statements<'tcx>(statements: &'tcx [Stmt<'tcx>]) -> impl Iterator<Item = &'tcx Expr<'tcx>> {
    statements
        .iter()
        .filter_map(|statement| match statement.kind {
            StmtKind::Expr(expression) | StmtKind::Semi(expression) => Some(expression),
            StmtKind::Let(local) => local.init,
            StmtKind::Item(_) => None,
        })
}

/// Whether evaluating `expr` is something the split can be told apart
/// from.
///
/// A call is where the answer comes from in both directions: it can leave
/// the closure, and it can have an effect whose order against the item's
/// steps the reader can see. An operator on a user type is a call too,
/// which is why the question is asked of typeck rather than of the node
/// kind alone.
///
/// Two builtins leave the closure without being a call of any kind: a
/// division or remainder by zero, in either the plain or the compound
/// spelling, and an index out of bounds. Arithmetic
/// overflow is the one left out, because a `fold` whose accumulator is
/// added to is the shape the chain trigger is mostly about, and
/// declining every one of those costs more than the panic ordering it
/// would buy.
fn observable<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    struct Observable<'a, 'tcx> {
        cx: &'a LateContext<'tcx>,
        found: bool,
    }
    impl<'tcx> Visitor<'tcx> for Observable<'_, 'tcx> {
        fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
            let leaves = self.cx.typeck_results().expr_ty(expr).is_never()
                || matches!(expr.kind, ExprKind::Match(_, _, MatchSource::TryDesugar(_)));
            let calls = matches!(
                expr.kind,
                ExprKind::Call(..)
                    | ExprKind::MethodCall(..)
                    | ExprKind::Index(..)
                    | ExprKind::Binary(
                        Spanned {
                            node: BinOpKind::Div | BinOpKind::Rem,
                            ..
                        },
                        ..
                    )
                    | ExprKind::AssignOp(
                        Spanned {
                            node: AssignOpKind::DivAssign | AssignOpKind::RemAssign,
                            ..
                        },
                        ..
                    ),
            ) || self
                .cx
                .typeck_results()
                .type_dependent_def_id(expr.hir_id)
                .is_some();
            if leaves || calls {
                self.found = true;
            }
            walk_expr(self, expr);
        }
    }
    let mut observable = Observable { cx, found: false };
    observable.visit_expr(expr);
    observable.found
}
