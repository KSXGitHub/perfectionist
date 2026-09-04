// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// Custom prefix is `["Hash", "Debug"]`. Traits not in this short
// prefix list — `Clone`, `Copy`, `PartialEq` — are alphabetised
// after the prefix-listed ones.

// Bad: the entries don't honour the custom prefix order
// (`Hash`, `Debug`, then alphabetised `Clone`, `Copy`, `PartialEq`).
#[derive(Debug, Clone, Hash, PartialEq, Copy)]
struct _Out;

// Good: prefix (`Hash`, `Debug`) first, then alphabetised tail.
#[derive(Hash, Debug, Clone, Copy, PartialEq)]
struct _Ok;

fn main() {}
