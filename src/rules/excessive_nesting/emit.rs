//! Shaping the diagnostic for one over-nested body.
//!
//! What makes a report lead to a genuine change is what it points at.
//! The note names the constructs on the way down, so the reader can
//! see which level to attack rather than only where the bottom is.
//! The helps then offer both directions honestly: flattening the
//! shape that is actually there, and extraction with the test that
//! tells a real extraction from a relocation.

use super::EXCESSIVE_NESTING;
use super::depth::{Construct, Deepest};
use crate::measured_fn::MeasuredFn;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_lint::{LateContext, LintContext};

/// Extraction is a first-class answer to deep nesting, so this never
/// argues against it — it states the property that separates one from
/// nesting relocated into an argument list.
const EXTRACTION_HELP: &str = "or extract the inner levels — name the new function for what \
                               it does, not where it came from; if it needs most of this \
                               function's locals as parameters, the nesting moved rather than \
                               went away";

/// The fallback when the deepest point is not a shape [`shape_hint`]
/// recognises.
const FLATTEN_HELP: &str = "return early with a guard clause or `let ... else` so the rest of \
                            the body stops being nested";

pub(super) fn emit(cx: &LateContext<'_>, function: &MeasuredFn, deepest: &Deepest, max: usize) {
    let kind = function.kind_label;
    let name = function.name;
    let depth = deepest.depth();
    let noun = if depth == 1 { "level" } else { "levels" };
    let message = format!("{kind} `{name}` nests {depth} {noun} deep, above the limit of {max}");
    let deepest_span = cx.sess().source_map().span_until_whitespace(deepest.span);
    span_lint_and_then(cx, EXCESSIVE_NESTING, function.span, message, |diag| {
        diag.span_note(
            deepest_span,
            format!("the deepest path is {}", path(deepest)),
        );
        diag.help(shape_hint(deepest).unwrap_or(FLATTEN_HELP));
        diag.help(EXTRACTION_HELP);
    });
}

/// The way down to the deepest point, outermost construct first.
fn path(deepest: &Deepest) -> String {
    deepest
        .path
        .iter()
        .map(|construct| construct.label())
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// A transform that removes a level from the shape at the bottom, when
/// the walk recorded enough about it to be sure the transform applies.
///
/// Each of these flattens in place, so none of them trades a level for
/// an argument list. `None` when the shape is not one of them, which
/// leaves the general guard-clause advice.
fn shape_hint(deepest: &Deepest) -> Option<&'static str> {
    let innermost = *deepest.path.last()?;
    match innermost {
        // An `if let` has one canonical flattening whatever encloses it.
        Construct::If { binds: true, .. } => {
            return Some(
                "an `if let` here: `let ... else` binds the same pattern and leaves early, \
                 so the rest of the block stops being nested",
            );
        }
        // The author already wrote the guard the general advice would
        // suggest; what is deep is the body they gave it.
        Construct::LetElse => {
            return Some(
                "the `else` body of a `let ... else` is the level here: it has to diverge, \
                 so keep it to the `return`, `break`, or `continue` that leaves",
            );
        }
        // A guard clause removes nothing from a block that is not a
        // condition in the first place.
        Construct::Block => {
            return Some(
                "a free-standing block is a level: drop the braces where they only scope a \
                 temporary, or give the block a name of its own",
            );
        }
        _ => {}
    }
    // Every hint below rewrites the innermost `if`'s condition, which
    // works only when it has no `else` branch to carry along.
    let Construct::If {
        has_else: false, ..
    } = innermost
    else {
        return None;
    };
    let parent = *deepest.path.get(deepest.depth().checked_sub(2)?)?;
    match parent {
        Construct::Match => Some(
            "an `if` inside a `match` arm: an arm guard folds the condition into the \
             pattern and removes this level",
        ),
        Construct::For | Construct::While | Construct::Loop => Some(
            "an `if` inside a loop: `continue` on the opposite condition removes this \
             level and unindents the body",
        ),
        // An `else`-less `if` has only a `then` branch, so the inner
        // `if` is certainly inside it and the two conditions merge.
        Construct::If {
            has_else: false, ..
        } => Some("an `if` directly inside an `if`: `&&` combines the two conditions"),
        _ => None,
    }
}
