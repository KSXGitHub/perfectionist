// Bad: `inner::their_macro!` is matched by the multi-segment entry
// `inner::their_macro` in `extra_macros`. The same machinery
// applies to a third-party macro referenced as
// `somecrate::their_macro!` — multi-segment entries tail-match the
// invocation path.
//
// Note: `#[macro_export]` here is a fixture-only workaround, not a
// representation of how a downstream crate would expose its macro.
// `macro_rules!` macros are not at crate-root scope by default, so
// `pub(crate) use crate::their_macro` would otherwise fail to
// resolve; `#[macro_export]` hoists the macro into the root namespace
// for the duration of this self-contained test. A real third-party
// macro is already exported through its defining crate's module
// structure, so users adding a qualified entry to `extra_macros`
// do not need the attribute.

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
