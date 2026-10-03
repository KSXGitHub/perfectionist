//! What a filtering adapter's answer depends on, and so which adapter a
//! lifted test may go in front of.
//!
//! Two rules lift a test out of a closure and have to put it somewhere:
//! one splits a conjunction, the other an `Option`-returning guard.
//! Both answer to the same constraint. An adapter answering by the
//! whole set and one answering by the leading run disagree the moment a
//! lifted filter drops the very item the other would have stopped at,
//! so a lifted test goes in an adapter of its own discipline or
//! nowhere.
//!
//! An adapter matching neither has no lift target, and the rules
//! naming it decline rather than pick one. `all` is the shape to keep
//! in mind: filtering first makes an item that failed the first test
//! vacuously fine, so the answer flips.

/// What an adapter's answer depends on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Discipline {
    /// Which items satisfy the test, and nothing else.
    Set,
    /// The leading run that satisfies it.
    Prefix,
}

impl Discipline {
    /// The adapter a lifted test goes in.
    pub(crate) fn lift_target(self) -> &'static str {
        match self {
            Self::Set => "filter",
            Self::Prefix => "take_while",
        }
    }
}
