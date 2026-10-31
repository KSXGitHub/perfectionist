//! What consumes a conversion's text, and whether it only ever
//! measures that text against string literals.
//!
//! The consumers that qualify measure the text and nothing else. An
//! equality names one literal; a `match` names several and may also
//! distinguish outcomes a byte comparison cannot, which is the
//! condition [`literal_patterns`] enforces.

use super::source::{Converted, converted};
use rustc_ast::LitKind;
use rustc_hir::{Arm, Expr, ExprKind, Pat, PatExpr, PatExprKind, PatKind, QPath};
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

/// One arm's pattern, and the literals it names in source order.
///
/// The arm's whole pattern is what the rewrite replaces, rather than
/// each literal inside it. `Some("a") | Some("b")` and
/// `Some("a" | "b")` name the same pair and want the same replacement,
/// and spans that nest cannot both be edited in one suggestion.
pub(super) struct LiteralArm {
    pub(super) pattern: Span,
    pub(super) literals: Vec<Symbol>,
}

/// How a conversion's text is measured against literals.
pub(super) enum Consumer {
    /// One `==` or `!=` against a single literal.
    Equality {
        /// The whole operand that carried the conversion. An `unwrap`
        /// the comparison applied sits inside it, so replacing this
        /// span drops that call along with the conversion.
        operand: Span,
        /// The whole operand that carried the literal, including the
        /// `Some` or `Ok` the comparison wrapped it in.
        comparand: Span,
        /// The literal the comparison names.
        literal: Symbol,
        /// Whether an `unwrap` or `expect` stood between the conversion
        /// and the comparison. The rewrite drops it, so input the
        /// conversion would have rejected answers `false` where it used
        /// to panic.
        drops_panic: bool,
    },
    /// A `match` whose arms name literals and wildcards.
    Patterns {
        /// Every arm that names literals.
        arms: Vec<LiteralArm>,
        /// The arm catching everything else, where it names the
        /// conversion's own failure as `None` or `Err(_)` rather than
        /// as `_`. A rewrite handing the `match` the value's own form
        /// hands it something that is neither an `Option` nor a
        /// `Result`, so that pattern has nothing left to name.
        failure: Option<Span>,
    },
}

impl Consumer {
    /// Whether the rewrite has to put its literals in pattern
    /// position.
    pub(super) fn needs_pattern(&self) -> bool {
        matches!(self, Consumer::Patterns { .. })
    }
}

/// `operand` read through [`converted`], with `other` as the literal it
/// is measured against.
///
/// The caller tries both operand orders, since either side may carry
/// the literal.
pub(super) fn literal_operand<'tcx>(
    cx: &LateContext<'tcx>,
    operand: &'tcx Expr<'tcx>,
    other: &'tcx Expr<'tcx>,
) -> Option<(&'tcx Expr<'tcx>, Converted<'tcx>, Consumer)> {
    let consumed = peel_unwrap(operand);
    let found = converted(cx, consumed)?;
    let literal = string_operand(other)?;
    Some((
        consumed,
        found,
        Consumer::Equality {
            operand: operand.span,
            comparand: other.span,
            literal,
            drops_panic: consumed.hir_id != operand.hir_id,
        },
    ))
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

/// `arms` read as the patterns a rewrite replaces, where each arm names
/// only literals and wildcards and the arms the rewrite cannot keep
/// apart already agree.
///
/// Answers `None` where any arm binds the decoded text, carries a
/// guard, or gives the non-UTF-8 case an outcome of its own. A byte
/// comparison separates a listed literal from everything else and
/// cannot separate ill-formed input from valid input that is none of
/// the literals, so those two have to reach one wildcard arm.
pub(super) fn literal_patterns(arms: &[Arm<'_>]) -> Option<Consumer> {
    let mut literal_arms = Vec::new();
    let mut wildcards = 0_usize;
    let mut failure = None;
    for arm in arms {
        let mut literals = Vec::new();
        match pattern_literals(arm.pat, &mut literals)? {
            Reach::Literals => literal_arms.push(LiteralArm {
                pattern: arm.pat.span,
                literals,
            }),
            Reach::Wildcard => wildcards += 1,
            Reach::Failure => {
                wildcards += 1;
                failure = Some(arm.pat.span);
            }
        }
    }
    // The non-literal cases reach one arm, so ill-formed input and
    // unlisted text already share an outcome. Two wildcard-shaped arms
    // mean the `match` told them apart, which the rewrite cannot.
    (wildcards == 1 && !literal_arms.is_empty()).then_some(Consumer::Patterns {
        arms: literal_arms,
        failure,
    })
}

/// What an arm's pattern reaches.
#[derive(Clone, Copy)]
enum Reach {
    /// Only the literals the pattern named, now pushed onto the
    /// caller's list.
    Literals,
    /// Everything the literal arms left over, named as `_`.
    Wildcard,
    /// Everything the literal arms left over, named as the
    /// conversion's own failure: `None` or `Err(_)`.
    Failure,
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
            let mut named_literals = false;
            let mut wildcard = false;
            let mut failure = false;
            for alternative in alternatives {
                match pattern_literals(alternative, literals)? {
                    Reach::Literals => named_literals = true,
                    Reach::Wildcard => wildcard = true,
                    Reach::Failure => failure = true,
                }
            }
            // An alternation naming both a literal and everything else
            // is one the rewrite cannot keep. A wildcard branch already
            // reaches every input, so the arm would answer for the arms
            // after it, which is not what the `match` did.
            if named_literals && (wildcard || failure) {
                return None;
            }
            Some(match (failure, wildcard) {
                (true, _) => Reach::Failure,
                (false, true) => Reach::Wildcard,
                (false, false) => Reach::Literals,
            })
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
            // comparison cannot make. Counting it among the wildcards
            // is what makes a `match` carrying both it and a `Some(_)`
            // arm answer `None` from `literal_patterns`.
            PatExprKind::Path(ref path) if is_none_path(path) => Some(Reach::Failure),
            _ => None,
        },
        PatKind::TupleStruct(ref path, [inner], _) if is_some_or_ok_path(path) => {
            pattern_literals(inner, literals)
        }
        // An `Err` arm is where a fallible conversion's report reaches
        // an outcome of its own, which the byte comparison cannot
        // distinguish from unlisted text.
        PatKind::TupleStruct(ref path, [inner], _)
            if is_err_path(path) && matches!(inner.kind, PatKind::Wild) =>
        {
            Some(Reach::Failure)
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
    // Negation is not tested: a string literal cannot carry one, and
    // the `LitKind::Str` test below excludes every literal that can.
    let PatExprKind::Lit { lit, .. } = pattern.kind else {
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
