//! Which `Iterator` adapters this rule speaks about, and where each
//! one's item parameter sits in the closure it takes.
//!
//! The condition an adapter has to meet is that **the item enters the
//! closure by value and never comes back out**. That is what makes a
//! leading `map` mean the same thing: an adapter borrowing the item, or
//! handing it back downstream, answers differently once a `map` has
//! replaced what it sees. `filter`, `find`, `take_while`,
//! `skip_while`, `max_by_key`, `partition` and `reduce` all fail it one
//! way or the other, and `inspect` fails it by passing the item on
//! unchanged.
//!
//! The table is a list rather than a type query because the condition
//! is about an adapter's contract, which no type exposes. Each entry
//! below was checked against that contract by hand.

use rustc_span::Symbol;

/// Where the adapter's closure sits, and which of its parameters is the
/// item.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    /// The closure takes the item alone, so the chain's last step stays
    /// with the adapter and the earlier ones lift into a `map`.
    Unary,
    /// The closure takes state and then the item, so the whole chain
    /// lifts and the body keeps the state expression. `fold`,
    /// `try_fold` and `scan` differ in what the state is and in what
    /// the closure returns, none of which bears on the item side.
    Binary,
}

impl Shape {
    /// Which of the closure's parameters carries the item.
    pub(super) fn item_parameter(self) -> usize {
        match self {
            Self::Unary => 0,
            Self::Binary => 1,
        }
    }

    /// Whether the chain's last step stays with the adapter rather than
    /// lifting into a `map`. It keeps the adapter holding a closure,
    /// which is what the pipeline's kind depends on.
    pub(super) fn keeps_the_last_step(self) -> bool {
        self == Self::Unary
    }
}

/// The shape of `method`'s closure, or `None` for a method this rule
/// does not speak about.
///
/// `fold` is also `Iterator::fold`'s name on several other traits this
/// rule does not reach, which the caller rules out by asking what the
/// call resolves to.
pub(super) fn shape(method: Symbol) -> Option<Shape> {
    // `filter_map`, `find_map` and `map_while` meet the condition and
    // are left out all the same: their closure returns an `Option`, and
    // what splits inside one is a chain of `Option` combinators, which
    // lifts into a discipline-matched adapter rather than into a `map`.
    // Flagging them here would offer the weaker split.
    Some(match method.as_str() {
        "map" | "flat_map" | "for_each" | "try_for_each" => Shape::Unary,
        "any" | "all" | "position" | "rposition" => Shape::Unary,
        "fold" | "try_fold" | "rfold" | "try_rfold" | "scan" => Shape::Binary,
        _ => return None,
    })
}
