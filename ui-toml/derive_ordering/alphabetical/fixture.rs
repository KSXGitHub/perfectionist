// Bad: `Debug` should come after `Clone` and `Copy` under
// alphabetical ordering.
#[derive(Debug, Clone, Copy)]
struct _Out;

// Good: already in ASCII-case-insensitive alphabetical order.
#[derive(Clone, Copy, Debug)]
struct _Ok;

// Good: a single derive is trivially sorted.
#[derive(Debug)]
struct _Single;

// Bad: `PartialEq` should come after `Hash` only if case-sensitively
// compared; under ASCII-case-insensitive comparison `Hash` < `PartialEq`,
// and the entries here are out of order.
#[derive(PartialEq, Eq, Hash)]
struct _MixedCase;

fn main() {}
