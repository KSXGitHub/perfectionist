#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, dead_code, unused_imports, reason = "ui fixture")]

pub mod thing {
    pub struct A;
    pub struct B;
}

pub mod api {
    pub use crate::thing::A;
}

pub mod prelude {
    pub use crate::thing::B;
}

// Flagged: `api` is now a recognised prelude segment name.
mod flagged {
    use crate::api::A;
}

// Not flagged: `prelude` is no longer in `prelude_segment_names`.
mod not_flagged {
    use crate::prelude::B;
}

fn main() {}
