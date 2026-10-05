//! The chain trigger: a chain of two or more steps rooted at the
//! closure's item, so one adapter performs all of them.
//!
//! The trigger walks *up* from the item parameter's occurrence, because
//! what makes a parent a step is that the chain so far is what it is
//! applied to; walking down would have to guess which sub-expression the
//! chain runs through. [`super::chain`] does that walk, [`super::adapter`]
//! says which methods are in scope and where their item sits, and
//! [`super::anchoring`] answers the one question whose wrong answer
//! compiles.
//!
//! Three gates stand between the walk and a diagnostic, and they fail
//! differently. The item occurring more than once leaves a split with a
//! binding to name that no longer exists, so the suggestion would be
//! `E0425`. A step whose result borrows the item would be `E0515` once
//! lifted. A chain in a conditionally-evaluated position, though, splits
//! into something that compiles and behaves differently, which is why
//! [`super::anchoring`] is where the care goes.

use super::adapter::{self, Adapter};
use super::chain::Step;
use super::{Call, Finding, Fix, anchoring, chain};
use crate::binding_uses::{names, uses};
use crate::common::{borrows, drops_a_comment};
use crate::exclusive_captures::exclusive;
use clippy_utils::source::snippet_with_applicability;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_errors::Applicability;
use rustc_hir::{CaptureBy, Closure, Expr, ExprKind, Pat};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::Symbol;

/// The chain this adapter's closure holds, where it is two or more steps
/// long.
pub(super) fn check<'tcx>(cx: &LateContext<'tcx>, call: &Call<'tcx>) -> Option<Finding> {
    let family = call.receiver.family;
    let (method, arguments) = (call.method, call.arguments);
    let adapter = adapter::adapter(cx, call)?;
    let argument = arguments.get(adapter.closure_argument)?;
    let ExprKind::Closure(closure) = argument.kind else {
        return None;
    };
    let body = cx.tcx.hir_body(closure.body);
    let parameter = body.params.get(adapter.item_parameter)?;
    let item = chain::binding(parameter.pat)?;
    // Each step gets its own closure after the split, so a second use of
    // the item would be left with nothing to name.
    let occurrences = uses(cx, body.value, &[item]);
    let [root] = *occurrences.as_slice() else {
        return None;
    };
    // A step moves into an adapter outside the closure, where nothing the
    // body declares is in scope, so a step naming any of it would be
    // `E0425` once lifted.
    let locals = chain::declared(body);
    let steps = chain::steps(cx, root, &locals);
    if steps.len() < 2 {
        return None;
    }
    // The last step stays with the adapter where the closure takes the
    // item alone, so only the ones moving out have to be liftable.
    let lifted = match adapter.keeps_the_last_step {
        true => &steps[..steps.len() - 1],
        false => &steps[..],
    };
    if !lifted.iter().all(|step| is_liftable(cx, step.expr)) {
        return None;
    }
    // A parallel adapter requires the item it produces to be `Send`,
    // where the folded form does not, because the value never leaves the
    // closure.
    if family.sends_between_threads() && !lifted.iter().all(|step| is_sendable(cx, step.expr)) {
        return None;
    }
    // A lifted step runs in a closure of its own, alongside the one the
    // adapter keeps. A capture only a shared borrow or a copy of a `Copy`
    // value lets both of them hold, so a step reaching any other one is
    // declined rather than worked out: whether the step is the only half
    // reaching it is a question the split's shape answers differently per
    // adapter.
    let held_alone = exclusive(cx, closure.def_id);
    if lifted.iter().any(|step| names(cx, step.expr, &held_alone)) {
        return None;
    }
    let top = steps.last().expect("two or more steps").expr;
    let anchored = match adapter.keeps_the_last_step {
        // Requiring the chain to *be* the body satisfies the position
        // condition vacuously, and holds the scope to a closure whose
        // whole job is the chain.
        true => top.hir_id == body.value.hir_id,
        false => {
            anchoring::always_evaluated(cx, top.hir_id, body.value.hir_id)
                && anchoring::nothing_observable_first(cx, top.hir_id, body.value.hir_id)
        }
    };
    if !anchored {
        return None;
    }
    Some(Finding {
        message: format!(
            "this closure chains {} steps onto the item, so `{method}` does all of them",
            steps.len(),
        ),
        help: help(adapter, method, lifted.len()),
        moves_the_receiver: !adapter.head_keeps_the_method,
        fix: fix(cx, call, closure, parameter.pat, &steps, adapter),
    })
}

/// What to tell the reader to do, which differs by whether the adapter
/// keeps a step and by whether the head of the split keeps the method.
fn help(adapter: Adapter, method: Symbol, lifted: usize) -> String {
    if adapter.head_keeps_the_method {
        return match lifted {
            1 => format!(
                "lift the first step into a leading `{method}` of its own, leaving \
                 the closure the step that is the adapter's",
            ),
            _ => format!(
                "lift the first step into a leading `{method}` and each later one \
                 into a `{}` of its own, leaving the closure the step that is the \
                 adapter's",
                adapter.lift_target,
            ),
        };
    }
    match adapter.keeps_the_last_step {
        true => format!(
            "lift all but the last step into a leading `{}` of its own, leaving \
             the closure the step that is the adapter's",
            adapter.lift_target,
        ),
        false => format!(
            "lift every step into a leading `{}` of its own, leaving the closure \
             the accumulation",
            adapter.lift_target,
        ),
    }
}

/// Whether lifting `step` out of the closure keeps it compiling.
///
/// A lifted step runs in a `map` of its own, applied to a local that
/// dies at the end of it, so a result borrowing that local is `E0515`.
/// Two shapes rule that out. A result carrying no lifetime borrows
/// nothing. And where what the step is applied to is itself a
/// reference, a `&self` borrows the referent rather than the local
/// pointer, and the referent outlives the closure:
/// `.map(|s: &str| s.trim())` hands back a borrow of what `s` points
/// at.
///
/// The question is about the step's own receiver, which is the item
/// only for the first step. Each later one is applied to the step
/// before it, so `.map(|s: &str| s.to_lowercase().trim())` has an
/// owned `String` under its `trim` however the item arrived.
///
/// A borrowing result is only the receiver's to answer for where nothing
/// else the step was handed could have lent it: `s.max(&String::from("m")[..])`
/// returns the shorter of two lifetimes, and the shorter one belongs to a
/// temporary the closure made. [`lends_nothing`] is that question.
///
/// Regions are erased by the time typeck results are read, so a
/// result's lifetime cannot be matched against the receiver's. These
/// clauses are the conservative answer that needs no such match.
fn is_liftable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    if !borrows(cx.typeck_results().expr_ty(step)) {
        return true;
    }
    let Some(receiver) = applied_to(step) else {
        return false;
    };
    if !cx.typeck_results().expr_ty(receiver).is_ref() {
        return false;
    }
    lends_nothing(cx, step)
}

/// Whether nothing `step` was handed besides its receiver could have lent
/// it the borrow its result carries.
///
/// A method call's arguments are read from the method's **declared**
/// signature rather than from the types the call instantiated. The
/// instantiated ones have their regions erased, where the signature still
/// names the regions and the type parameters each input and the output are
/// made of, so the question is whether the output shares any of them with
/// an argument. `str::strip_prefix<P>(&self, prefix: P) -> Option<&str>`
/// shares none: its result is made of the receiver's region, and the
/// pattern is a parameter of its own. `Ord::max(self, other: Self) -> Self`
/// shares `Self` with its argument, which is how it returns the shorter of
/// two lifetimes.
///
/// A call's sole argument is the chain, which is the receiver here, so
/// what is left to ask about is the callee: an expression yielding an
/// `Fn` that returns a borrow lends that borrow to the result. A path to
/// a function is not one, its regions belonging to the signature rather
/// than to anything the zero-sized item holds.
fn lends_nothing<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    match step.kind {
        ExprKind::MethodCall(..) => arguments_lend_nothing(cx, step),
        ExprKind::Call(callee, _) if !matches!(callee.kind, ExprKind::Path(_)) => {
            !borrows(cx.typeck_results().expr_ty(callee))
        }
        _ => true,
    }
}

/// Whether the declared signature of the method `step` calls makes its
/// output of nothing an argument carries.
fn arguments_lend_nothing<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    let Some(method) = cx.typeck_results().type_dependent_def_id(step.hir_id) else {
        return false;
    };
    let signature = cx.tcx.fn_sig(method).skip_binder().skip_binder();
    // The receiver is the step's own, which `is_liftable` has answered for.
    let [_receiver, arguments @ ..] = signature.inputs() else {
        return false;
    };
    let output = made_of(signature.output());
    !arguments
        .iter()
        .flat_map(|argument| made_of(*argument))
        .any(|part| output.contains(&part))
}

/// The regions and type parameters `ty` is made of, which is what two
/// declared types share where a value of the one can be made out of a
/// value of the other.
fn made_of<'tcx>(ty: Ty<'tcx>) -> Vec<ty::GenericArg<'tcx>> {
    ty.walk()
        .filter(|part| match part.kind() {
            ty::GenericArgKind::Lifetime(_) => true,
            ty::GenericArgKind::Type(inner) => matches!(inner.kind(), ty::Param(_)),
            ty::GenericArgKind::Const(_) => false,
        })
        .collect()
}

/// What `step` applies itself to: a method call's receiver, or the sole
/// argument of a call.
fn applied_to<'tcx>(step: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    match step.kind {
        ExprKind::MethodCall(_, receiver, ..) => Some(receiver),
        ExprKind::Call(_, [only]) => Some(only),
        _ => None,
    }
}

/// Whether `step`'s result can cross a thread boundary.
///
/// Measured on the planning file's own example:
/// `.map(|s| Rc::new(*s).len())` compiles and
/// `.map(|s| Rc::new(*s)).map(|r| r.len())` is `E0277`.
fn is_sendable<'tcx>(cx: &LateContext<'tcx>, step: &'tcx Expr<'tcx>) -> bool {
    let Some(send) = cx.tcx.get_diagnostic_item(sym::Send) else {
        return false;
    };
    implements_trait(cx, cx.typeck_results().expr_ty(step), send, &[])
}

/// The rewrite: one `lift_target` per step that moves out, leaving the
/// adapter the step that was always its own.
///
/// A step is written as the function it already is wherever that function
/// can be named, which takes the question of a parameter name away rather
/// than answering it. The item's own name describes what the *first* step is
/// handed and nothing above it: `|line| line.trim()` reads, and the `|line|`
/// a second step would take is bound to a length.
///
/// So the first step may fall back to a closure naming the item, which is
/// the name it had. A later step falls back to a placeholder instead, and
/// the rewrite is then offered for the reader to finish rather than applied.
///
/// The shapes left to the help text are an adapter whose chain leaves whole,
/// which keeps a closure whose body has to be rewritten rather than
/// replaced, and a piping method, which puts the first step in its own
/// method instead of in `lift_target`.
fn fix<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Call<'tcx>,
    closure: &'tcx Closure<'tcx>,
    parameter: &'tcx Pat<'tcx>,
    steps: &[Step<'tcx>],
    adapter: Adapter,
) -> Option<Fix> {
    if !adapter.keeps_the_last_step || adapter.head_keeps_the_method {
        return None;
    }
    let (last, lifted) = steps.split_last()?;
    let mut applicability = Applicability::MachineApplicable;
    let moves = match closure.capture_clause {
        CaptureBy::Value { .. } => "move ",
        _ => "",
    };
    let item =
        snippet_with_applicability(cx, parameter.span, "..", &mut applicability).into_owned();
    let mut written = Vec::with_capacity(steps.len());
    let targets = lifted
        .iter()
        .map(|step| (step, adapter.lift_target))
        .chain([(last, call.method.as_str())]);
    for (index, (step, adapter_name)) in targets.enumerate() {
        let written_step = match point_free(cx, step.expr, &mut applicability) {
            Some(function) => function,
            None => {
                // The first step is handed the item, so the item's name is
                // the one it had. Above it the name would describe the wrong
                // value, and a placeholder says so rather than guessing.
                let name = match index {
                    0 => item.clone(),
                    _ => {
                        applicability = Applicability::HasPlaceholders;
                        PLACEHOLDER.to_owned()
                    }
                };
                let body = over_the_item(cx, step.expr, &name, &mut applicability)?;
                format!("{moves}|{name}| {body}")
            }
        };
        written.push(format!("{adapter_name}({written_step})"));
    }
    if drops_a_comment(cx, call.fix_span, steps.iter().map(|step| step.expr.span)) {
        applicability = Applicability::Unspecified;
    }
    Some(Fix {
        span: call.fix_span,
        suggestion: written.join("."),
        applicability,
    })
}

/// What stands in for a name only the reader can choose.
const PLACEHOLDER: &str = "/* name */";

/// The function a step already is, named so the adapter can be handed it
/// directly, or `None` where naming it would ask for a different type from
/// the one the step was handed.
///
/// A call around the chain hands its callee exactly what the chain evaluated
/// to, so the callee is that function whatever its signature.
///
/// A method call is the case needing care. `String::len` takes `&String`
/// where `str::len` takes `&str`, and a `String` reaches the latter only
/// through a deref the path form does not perform, so the method's self type
/// is compared against what the receiver actually is. That is what declines
/// the path form for an item of `String` while allowing it for one of `&str`.
fn point_free<'tcx>(
    cx: &LateContext<'tcx>,
    step: &'tcx Expr<'tcx>,
    applicability: &mut Applicability,
) -> Option<String> {
    match step.kind {
        ExprKind::Call(callee, [_]) if matches!(callee.kind, ExprKind::Path(_)) => {
            Some(snippet_with_applicability(cx, callee.span, "..", applicability).into_owned())
        }
        ExprKind::MethodCall(segment, receiver, [], _) => {
            let method = cx.typeck_results().type_dependent_def_id(step.hir_id)?;
            // A trait method needs the trait in scope where the path is
            // written, which the folded form never had to be.
            if cx.tcx.trait_of_assoc(method).is_some() {
                return None;
            }
            let arguments = cx.typeck_results().node_args(step.hir_id);
            let signature = cx.tcx.fn_sig(method).instantiate(cx.tcx, arguments);
            let [self_ty, ..] = *signature.skip_binder().inputs() else {
                return None;
            };
            // Shapes rather than types, because a signature's lifetimes are
            // bound where a receiver's are whatever it was written with, and
            // erasing regions does not reach a bound one. The borrow depth
            // and the type the path names are what decide whether the path
            // form takes what the step was handed.
            let taken = shape(cx, self_ty)?;
            let held = shape(cx, cx.typeck_results().expr_ty(receiver))?;
            if taken != held {
                return None;
            }
            let owner = taken.1;
            // A path takes its generic arguments turbofished where a method
            // call writes them bare, and the span covers what was written.
            let turbofish = segment.args.map_or_else(String::new, |arguments| {
                let written = snippet_with_applicability(cx, arguments.span_ext, "", applicability);
                match written.starts_with("::") {
                    true => written.into_owned(),
                    false => format!("::{written}"),
                }
            });
            Some(format!("{owner}::{}{turbofish}", segment.ident.name))
        }
        _ => None,
    }
}

/// How deeply `ty` is borrowed, and the path a method on it is written
/// under, so `&str` answers one borrow of `str`.
///
/// References are peeled because a method taking `&self` is written under
/// the type it borrows rather than under the borrow, and counted because
/// that is what says whether the path form takes what the step was handed:
/// `String` reaches `str::len` through a deref, and answers zero borrows of
/// `String` against the one borrow of `str` that path takes.
fn shape(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<(usize, String)> {
    let mut peeled = ty;
    let mut depth = 0;
    while let ty::Ref(_, referent, _) = peeled.kind() {
        peeled = *referent;
        depth += 1;
    }
    let path = match peeled.kind() {
        ty::Str => "str".to_owned(),
        ty::Bool => "bool".to_owned(),
        ty::Char => "char".to_owned(),
        ty::Int(kind) => kind.name_str().to_owned(),
        ty::Uint(kind) => kind.name_str().to_owned(),
        ty::Float(kind) => kind.name_str().to_owned(),
        ty::Adt(definition, _) if !definition.is_box() => {
            cx.tcx.item_name(definition.did()).to_string()
        }
        _ => return None,
    };
    Some((depth, path))
}

/// A step written as though `name` were what it is applied to, so
/// `<chain>.len()` becomes `line.len()`.
///
/// The step's own text is spliced rather than rebuilt, which keeps a
/// turbofish and whatever spacing the reader wrote. The span the receiver
/// occupies lies inside the step's own, neither being from an expansion,
/// which is what makes the offsets into the snippet the right ones.
fn over_the_item<'tcx>(
    cx: &LateContext<'tcx>,
    step: &'tcx Expr<'tcx>,
    name: &str,
    applicability: &mut Applicability,
) -> Option<String> {
    let inner = match step.kind {
        ExprKind::MethodCall(_, receiver, ..) => receiver,
        ExprKind::Call(_, [only]) => only,
        _ => return None,
    };
    let (whole, held) = (step.span, inner.span);
    if whole.from_expansion() || held.from_expansion() {
        return None;
    }
    if held.lo() < whole.lo() || held.hi() > whole.hi() {
        return None;
    }
    let text = snippet_with_applicability(cx, whole, "..", applicability).into_owned();
    let start = usize::try_from((held.lo() - whole.lo()).0).ok()?;
    let end = usize::try_from((held.hi() - whole.lo()).0).ok()?;
    if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    Some(format!("{}{name}{}", &text[..start], &text[end..]))
}
