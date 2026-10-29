//! What consumes a conversion's text, and whether it only ever
//! measures that text against string literals.
//!
//! Two consumers qualify. An equality names one literal; a `match`
//! names several and may also distinguish outcomes a byte comparison
//! cannot, which is the condition [`literal_arms`] enforces.

use super::source::{Converted, converted};
use rustc_ast::LitKind;
use rustc_hir::{Arm, Expr, ExprKind, Pat, PatExpr, PatExprKind, PatKind, QPath};
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// How a conversion's text is measured against literals.
pub(super) enum Consumer {
    /// One `==` or `!=` against a single literal.
    Equality {
        /// The literal the comparison names.
        literal: Symbol,
    },
    /// A `match` whose arms name literals and wildcards.
    Patterns {
        /// Every literal any arm names.
        literals: Vec<Symbol>,
    },
}

impl Consumer {
    /// Every literal this consumer measures the text against.
    pub(super) fn literals(&self) -> &[Symbol] {
        match self {
            Consumer::Equality { literal } => core::slice::from_ref(literal),
            Consumer::Patterns { literals } => literals,
        }
    }

    /// Whether the rewrite has to put its literals in pattern
    /// position, where no `OsStr` literal exists to put.
    pub(super) fn needs_pattern(&self) -> bool {
        matches!(self, Consumer::Patterns { .. })
    }
}

/// `operand` read through [`converted`] and `other` as the literal it
/// is measured against.
///
/// The caller tries both operand orders, since either side may carry
/// the literal.
pub(super) fn literal_operand<'tcx>(
    cx: &LateContext<'tcx>,
    operand: &'tcx Expr<'tcx>,
    other: &'tcx Expr<'tcx>,
) -> Option<(&'tcx Expr<'tcx>, Converted<'tcx>, Symbol)> {
    let consumed = peel_unwrap(operand);
    let found = converted(cx, consumed)?;
    Some((consumed, found, string_operand(other)?))
}

/// The string a comparison's literal operand carries.
///
/// A `Some` or an `Ok` around the literal is peeled: the conversion
/// yields an `Option` or a `Result`, so that wrapper is how the
/// comparison reaches the text rather than part of what it asks.
///
/// A literal out of an expansion answers `None`. The help reads the
/// literal's own text, so `value.to_str() == Some(wanted!())` would
/// otherwise be described in terms of whatever that expands to today.
fn string_operand(expr: &Expr<'_>) -> Option<Symbol> {
    let inner = match expr.kind {
        ExprKind::Call(callee, [argument]) if is_some_or_ok(callee) => argument,
        _ => expr,
    };
    if inner.span.from_expansion() {
        return None;
    }
    let ExprKind::Lit(literal) = inner.kind else {
        return None;
    };
    match literal.node {
        LitKind::Str(value, _) => Some(value),
        _ => None,
    }
}

/// Whether `callee` names the `Some` or `Ok` constructor.
fn is_some_or_ok(callee: &Expr<'_>) -> bool {
    let ExprKind::Path(QPath::Resolved(_, path)) = callee.kind else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| matches!(segment.ident.name.as_str(), "Some" | "Ok"))
}

/// Every literal `arms` names, where each arm names only literals and
/// wildcards and the arms the rewrite cannot keep apart already agree.
///
/// Answers `None` where any arm binds the decoded text, carries a
/// guard, or gives the non-UTF-8 case an outcome of its own. A byte
/// comparison separates a listed literal from everything else and
/// cannot separate ill-formed input from valid input that is none of
/// the literals, so those two have to reach one wildcard arm.
pub(super) fn literal_arms(arms: &[Arm<'_>]) -> Option<Vec<Symbol>> {
    let mut literals = Vec::new();
    let mut wildcards = 0_usize;
    for arm in arms {
        // A guard reads the binding its pattern made, or some other
        // value, and either way the arm is deciding on more than which
        // literal the text is.
        if arm.guard.is_some() {
            return None;
        }
        match pattern_literals(arm.pat, &mut literals)? {
            Reach::Literals => {}
            Reach::Wildcard => wildcards += 1,
        }
    }
    // The non-literal cases reach one arm, so ill-formed input and
    // unlisted text already share an outcome. Two wildcard-shaped arms
    // mean the `match` told them apart, which the rewrite cannot.
    (wildcards == 1 && !literals.is_empty()).then_some(literals)
}

/// What an arm's pattern reaches.
enum Reach {
    /// Only the literals the pattern named, now pushed onto the
    /// caller's list.
    Literals,
    /// Everything the literal arms left over.
    Wildcard,
}

/// Push every literal `pat` names onto `literals`, answering what the
/// pattern reaches.
///
/// `None` says the pattern is one the rewrite cannot keep: it binds the
/// text, or names something other than a string literal.
fn pattern_literals(pat: &Pat<'_>, literals: &mut Vec<Symbol>) -> Option<Reach> {
    match pat.kind {
        PatKind::Wild => Some(Reach::Wildcard),
        PatKind::Or(alternatives) => {
            // An alternation is a wildcard as a whole where any branch
            // is one, since the arm then catches what the others left.
            let mut reach = Reach::Literals;
            for alternative in alternatives {
                if matches!(pattern_literals(alternative, literals)?, Reach::Wildcard) {
                    reach = Reach::Wildcard;
                }
            }
            Some(reach)
        }
        // A unit variant and a literal are both `PatKind::Expr`, so
        // which one this is decided by the `PatExpr` inside.
        PatKind::Expr(pattern) => match pattern.kind {
            PatExprKind::Lit { .. } => {
                literals.push(string_pattern(pattern)?);
                Some(Reach::Literals)
            }
            // A `None` arm is where the conversion's report reaches an
            // outcome of its own, which is the distinction the byte
            // comparison cannot make. Counting it as a wildcard is what
            // makes a `match` carrying both it and a `Some(_)` arm
            // answer `None` from `literal_arms`.
            PatExprKind::Path(ref path) if is_none_path(path) => Some(Reach::Wildcard),
            _ => None,
        },
        PatKind::TupleStruct(ref path, [inner], _) if is_some_or_ok_path(path) => {
            pattern_literals(inner, literals)
        }
        // An `Err` arm reaches an outcome of its own for the same
        // reason a `None` arm does.
        PatKind::TupleStruct(ref path, [inner], _)
            if is_err_path(path) && matches!(inner.kind, PatKind::Wild) =>
        {
            Some(Reach::Wildcard)
        }
        _ => None,
    }
}

/// The string a literal pattern carries, where it carries one written
/// by the author.
fn string_pattern(pattern: &PatExpr<'_>) -> Option<Symbol> {
    if pattern.span.from_expansion() {
        return None;
    }
    let PatExprKind::Lit {
        lit,
        negated: false,
    } = pattern.kind
    else {
        return None;
    };
    match lit.node {
        LitKind::Str(value, _) => Some(value),
        _ => None,
    }
}

/// Whether `path` names the `Some` or `Ok` constructor in a pattern.
fn is_some_or_ok_path(path: &QPath<'_>) -> bool {
    last_segment_is(path, &["Some", "Ok"])
}

/// Whether `path` names the `None` constructor in a pattern.
fn is_none_path(path: &QPath<'_>) -> bool {
    last_segment_is(path, &["None"])
}

/// Whether `path` names the `Err` constructor in a pattern.
fn is_err_path(path: &QPath<'_>) -> bool {
    last_segment_is(path, &["Err"])
}

/// Whether `path`'s final segment is one of `names`.
fn last_segment_is(path: &QPath<'_>, names: &[&str]) -> bool {
    let QPath::Resolved(_, path) = path else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| names.contains(&segment.ident.name.as_str()))
}

/// `expr` with an `unwrap` or `expect` peeled off.
///
/// Those sit around the conversion rather than replacing it: the text
/// they hand back is the text the conversion produced, so the
/// comparison beneath them is still measuring the converted value.
pub(super) fn peel_unwrap<'tcx>(expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    let ExprKind::MethodCall(segment, receiver, arguments, _) = expr.kind else {
        return expr;
    };
    let peelable =
        matches!(segment.ident.name.as_str(), "unwrap" | "expect") && arguments.len() <= 1;
    if peelable { receiver } else { expr }
}
