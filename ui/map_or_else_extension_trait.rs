// edition:2024

// Regression test: `map_or_else` must not fire on a call to an
// extension trait's method of the same name. `BorrowedMapOrElse` lends
// `map_or_else` to a borrowed `Option`, where the inherent method
// cannot apply: it takes `self`, and a `String` cannot be moved out of
// a `&Option<String>`. The call therefore resolves to the trait's
// method, whose arguments mean whatever the trait says they mean.
//
// The receiver is still an `Option` and the method still wears the
// name, so every other part of the trigger admits this call: asking
// whether the method is an inherent one is the only thing stopping the
// diagnostic. The trait's impl covers every `&Option<_>`, which is why
// it lives here rather than beside `through_a_reference` in
// `ui/map_or_else.rs`.

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

// Not flagged: the call is `BorrowedMapOrElse`'s rather than the inherent
// `map_or_else`, which cannot take a `String` out of a borrow.
fn through_a_trait(name: &Option<String>) -> usize {
    name.map_or_else(|| 0, |name| name.len())
}

fn main() {}
