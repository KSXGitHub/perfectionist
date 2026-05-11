// Bad: `inner::their_macro!` is matched by the multi-segment entry
// `inner::their_macro` in `extra_name_based`. The same machinery
// applies to a third-party macro referenced as
// `somecrate::their_macro!` — multi-segment entries tail-match the
// invocation path. The fixture defines `their_macro!` with
// `#[macro_export]` (which puts it in the crate root's item
// namespace) and re-exports it through a local `mod inner` only
// because the test must be self-contained.

#[macro_export]
macro_rules! their_macro {
    ($($item:expr),* $(,)?) => {{ $(let _ = $item;)* 0 }};
}

mod inner {
    pub(crate) use crate::their_macro;
}

fn main() {
    let _ = inner::their_macro!(
        1,
        2,
        3
    );
}
