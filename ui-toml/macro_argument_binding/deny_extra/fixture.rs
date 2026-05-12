// `deny_extra` adds a project-specific macro to the deny list. The
// `inner::their_macro!` invocation here matches the multi-segment
// entry `inner::their_macro` — multi-segment entries tail-match the
// invocation path, so it covers both `their_macro!` reached through
// the `inner` module here and a third-party macro reached as
// `somecrate::their_macro!`.

#[macro_export]
macro_rules! their_macro {
    ($item:expr) => {{
        let _ = $item;
        0
    }};
}

mod inner {
    pub(crate) use crate::their_macro;
}

fn main() {
    let _ = inner::their_macro!(value());
}

fn value() -> u32 {
    0
}
