// edition:2024

// Regression test: `map_or_else_usage` must not fire on a call to an
// extension trait's method of the same name. The trait below lends
// `map_or_else` to a borrowed `Option`, where the inherent method
// cannot apply — it takes `self`, and a `String` cannot be moved out of
// a `&Option<String>` — so the call resolves to the trait's method,
// whose arguments mean whatever the trait says they mean.
//
// The receiver is still an `Option` and the method still wears the
// name, so every other half of the trigger admits this call: asking
// whether the method is an inherent one is the only thing stopping the
// diagnostic. The trait's impl covers every `&Option<_>`, which is why
// it lives here rather than beside the fixture cases that reach the
// inherent method through a deref.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

trait BorrowedMapOrElse<Held> {
    fn map_or_else<Mapped>(
        self,
        default: impl FnOnce() -> Mapped,
        mapper: impl FnOnce(&Held) -> Mapped,
    ) -> Mapped;
}

impl<Held> BorrowedMapOrElse<Held> for &Option<Held> {
    fn map_or_else<Mapped>(
        self,
        default: impl FnOnce() -> Mapped,
        mapper: impl FnOnce(&Held) -> Mapped,
    ) -> Mapped {
        match self {
            Some(held) => mapper(held),
            None => default(),
        }
    }
}

fn through_a_trait(name: &Option<String>) -> usize {
    name.map_or_else(|| 0, |name| name.len())
}

fn main() {}
