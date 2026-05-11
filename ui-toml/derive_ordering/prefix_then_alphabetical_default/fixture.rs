// Default prefix is `[Debug, Default, Clone, Copy, PartialEq, Eq,
// PartialOrd, Ord, Hash]`. Every derive used below is in the
// prefix, so the rule enforces the prefix's exact order without
// any alphabetised tail.

// Bad: `Clone` before `Debug` violates the prefix order.
#[derive(Clone, Debug)]
struct _Out;

// Good: prefix order honoured.
#[derive(Debug, Clone, Copy)]
struct _Ok;

// Bad: `Hash` should come last (after `Ord`).
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
struct _HashFirst;

fn main() {}
