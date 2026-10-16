//! What a filtering adapter's answer depends on, and so which adapter
//! work lifted out of its closure may go in.
//!
//! Three rules lift something out of a closure and have to put it
//! somewhere: one splits a conjunction, one an `Option`-returning guard,
//! and one a stage that may fail. All answer to the same constraint. An adapter answering by the
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

    /// The adapter a lifted stage that may fail goes in, which keeps one
    /// stage and yields a stream.
    ///
    /// A stage drops the item where it fails, so it answers to this
    /// module's constraint exactly as a test does: under a prefix-shaped
    /// adapter a `filter_map` would drop the very item the run would have
    /// stopped at, where a `map_while` stops there.
    pub(crate) fn fallible_target(self) -> &'static str {
        match self {
            Self::Set => "filter_map",
            Self::Prefix => "map_while",
        }
    }
}
